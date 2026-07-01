use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use typst::diag::{FileError, FileResult, Severity, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Datetime, Duration, Label, Repr, Value};
use typst::introspection::Introspector;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
// `WorldExt` lives in `typst::foundations` but is re-exported from the
// crate root only in 0.15+. Importing via the crate root keeps the
// dependency on a specific module-version bump out of the way.
use typst::WorldExt;
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryExt, World};
use typst_html::{HtmlDocument, HtmlOptions};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{FontPath, FontSource, FontStore};
use typst_utils::PicoStr;

// ── Process-wide caches ───────────────────────────────────────────────
//
// `Library` is essentially static for our purposes — we only ever enable
// the `Html` feature, with no per-call feature variation — so rebuilding
// it on every `compile` call is pure waste.
//
// `typst-kit` font discovery walks `/Library/Fonts`, `/usr/share/fonts`,
// and friends. On macOS the first call can take 200–500 ms; the result is
// effectively immutable for the lifetime of the process, so cache it.
//
// Both caches are bounded to the configured feature set / system font
// set; per-instance and per-call `fontPaths` are layered on top inside
// `TyHtml::new` / `TyHtml::compile` / `TyHtml::compileSync`.

static LIBRARY: OnceLock<LazyHash<Library>> = OnceLock::new();

/// System font entries cached for the lifetime of the process.
///
/// We cache `(CachedFontPath, FontInfo)` pairs (the public surface of
/// `typst_kit::fonts::system()`) rather than `FontSlot` (private to
/// `FontStore`) or a `FontStore` itself (`FontStore` is not `Clone`,
/// `FontPath` is not `Clone`, and `Arc<T>: FontSource` is not
/// blanket-impl'd by `typst-kit`). The `CachedFontPath` newtype bridges
/// that gap: it owns an `Arc<FontPath>`, is itself `Clone` (refcount
/// bump), and delegates `load` to the inner `FontPath`. Per call we
/// clone the cached slice and extend a fresh `FontStore`, avoiding
/// the multi-hundred-millisecond filesystem scan that
/// `typst_kit::fonts::system()` performs on first call.
#[derive(Clone)]
struct CachedFontPath(Arc<FontPath>);

impl FontSource for CachedFontPath {
    fn load(&self) -> Option<Font> {
        self.0.load()
    }
}

static SYSTEM_FONT_ENTRIES: OnceLock<Vec<(CachedFontPath, FontInfo)>> = OnceLock::new();

fn library() -> &'static LazyHash<Library> {
    LIBRARY.get_or_init(|| {
        let features = Features::from_iter(std::iter::once(Feature::Html));
        LazyHash::new(Library::builder().with_features(features).build())
    })
}

fn system_font_entries() -> &'static [(CachedFontPath, FontInfo)] {
    SYSTEM_FONT_ENTRIES.get_or_init(|| {
        typst_kit::fonts::system()
            .map(|(p, i)| (CachedFontPath(Arc::new(p)), i))
            .collect()
    })
}

// ── JS-facing types ────────────────────────────────────────────────────

/// Options for the [`TyHtml`] constructor.
///
/// These are scanned exactly once, at construction time, and merged with the
/// system font set on the instance. Per-call font additions live on
/// [`CompileOptions::font_paths`].
#[napi(object)]
pub struct TyHtmlOptions {
    /// Extra font directories to scan at construction time (in addition to
    /// the system font set). Each is scanned exactly once; the resulting
    /// entries are stored on the instance.
    pub font_paths: Option<Vec<String>>,
}

/// Options for a single `compile` / `compileSync` call.
#[napi(object)]
pub struct CompileOptions {
    /// Strip everything outside `<body>…</body>` (no `<!DOCTYPE>`, `<html>`, `<head>`).
    pub body_only: Option<bool>,
    /// Pretty-print the HTML output with indentation.
    pub pretty: Option<bool>,
    /// Skip metadata extraction (faster, no `<meta>` query).
    pub no_metadata: Option<bool>,
    /// Label to query for metadata. Defaults to `"meta"`.
    pub metadata_label: Option<String>,
    /// Additional font directories for *this call only*, layered on top of
    /// the [`TyHtmlOptions::font_paths`] registered at construction time.
    pub font_paths: Option<Vec<String>>,
}

/// A single Typst warning emitted during compilation.
#[napi(object)]
pub struct CompileWarning {
    pub message: String,
}

/// Severity of a [`CompileDiagnostic`]. The variants are lowercase on
/// purpose: `#[napi(string_enum)]` emits the variant name verbatim as
/// the JS string, and the public TS API uses `'warning' | 'error'`
/// (per the feature roadmap). Rust convention would have these
/// `PascalCase` — override that here so the wire format matches.
#[allow(non_camel_case_types)]
#[napi(string_enum)]
pub enum CompileDiagnosticSeverity {
    warning,
    error,
}

/// A single warning or error from the Typst compile pipeline, with
/// structured location info where Typst can supply it.
///
/// The location fields (`file`, `line`, `column`) are optional —
/// Typst emits synthetic diagnostics for things like "html export
/// is under active development" that have no source span.
/// `severity` distinguishes warnings (non-fatal) from errors
/// (compile halted, `html` will be empty).
#[napi(object)]
pub struct CompileDiagnostic {
    pub severity: CompileDiagnosticSeverity,
    pub message: String,
    /// Absolute path of the file the diagnostic originates from, if
    /// the span resolves to a real source file. Currently only the
    /// main input file is recognised — package / import files are
    /// reported without `file` / `line` / `column`.
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

/// Result of a compilation attempt.
///
/// `html` is the rendered output, or an empty string if the compile
/// halted (errors are then in `diagnostics` with `severity: 'error'`).
/// `metadata` is a JSON-encoded string (e.g. `'{"title":"..."}'`);
/// `null` when no `<meta>` label is present or `noMetadata: true` was
/// passed. `diagnostics` is the full list of warnings and errors with
/// structured span info. `warnings` is kept for backwards compat and
/// is the message-only projection of the `severity: 'warning'`
/// entries from `diagnostics`.
#[napi(object)]
pub struct CompileResult {
    pub html: String,
    pub metadata: Option<String>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub warnings: Vec<CompileWarning>,
}

/// Compile options with all `Option`s resolved to concrete values.
struct FlattenedOptions {
    body_only: bool,
    pretty: bool,
    no_metadata: bool,
    label_name: String,
    extra_font_paths: Vec<PathBuf>,
}

impl CompileOptions {
    fn flatten_or_default(options: Option<CompileOptions>) -> FlattenedOptions {
        match options {
            Some(opts) => FlattenedOptions {
                body_only: opts.body_only.unwrap_or(false),
                pretty: opts.pretty.unwrap_or(false),
                no_metadata: opts.no_metadata.unwrap_or(false),
                label_name: opts.metadata_label.unwrap_or_else(|| "meta".to_string()),
                extra_font_paths: opts
                    .font_paths
                    .unwrap_or_default()
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
            },
            None => FlattenedOptions {
                body_only: false,
                pretty: false,
                no_metadata: false,
                label_name: "meta".to_string(),
                extra_font_paths: Vec::new(),
            },
        }
    }
}

// ── File loader ────────────────────────────────────────────────────────

struct BridgeFiles {
    main: FileId,
    /// Absolute path of the main input file. Kept alongside `main`
    /// (the FileId) so diagnostic builders can surface the original
    /// path on the JS side without re-canonicalising.
    main_path: PathBuf,
    project: FsRoot,
}

impl BridgeFiles {
    fn new(root: PathBuf, main_abs: PathBuf) -> Result<Self> {
        // `main_abs` must already be canonicalized by the caller — running
        // `Path::canonicalize` here would duplicate work done in
        // `BridgeWorld::new` and on Windows it touches the filesystem.
        let vpath = VirtualPath::virtualize(&root, &main_abs)
            .map_err(|e| anyhow::anyhow!("Failed to virtualize path: {e}"))?;
        let main = RootedPath::new(VirtualRoot::Project, vpath).intern();

        Ok(Self {
            main,
            main_path: main_abs,
            project: FsRoot::new(root),
        })
    }
}

impl FileLoader for BridgeFiles {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        let root = match id.root() {
            VirtualRoot::Project => self.project.clone(),
            VirtualRoot::Package(spec) => {
                let dir = package_dir_for(spec)
                    .ok_or_else(|| FileError::NotFound(id.vpath().get_with_slash().into()))?;
                FsRoot::new(dir)
            }
        };
        root.load(id.vpath())
    }
}

/// Resolve a package spec to its local directory on disk.
fn package_dir_for(spec: &typst::syntax::package::PackageSpec) -> Option<PathBuf> {
    for base in [typst_data_dir(), typst_cache_dir()] {
        let path = base
            .join("packages")
            .join(spec.namespace.as_str())
            .join(spec.name.as_str())
            .join(spec.version.to_string());
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

fn typst_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("typst")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("typst")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(dir).join("typst")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("typst")
        }
    }
}

fn typst_cache_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("typst")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home)
            .join("Library")
            .join("Caches")
            .join("typst")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(dir).join("typst")
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            PathBuf::from(home).join(".cache").join("typst")
        }
    }
}

// ── World ──────────────────────────────────────────────────────────────

struct BridgeWorld {
    /// Borrowed from the process-wide `LIBRARY` cache — see module-level docs.
    library: &'static LazyHash<Library>,
    /// Owned per call: cloned from the cached system store (cheap — Arc
    /// refcount bumps plus a small `Vec<FontInfo>`) and then extended with
    /// any caller-supplied `fontPaths`.
    fonts: FontStore,
    files: FileStore<BridgeFiles>,
    now: Time,
}

impl BridgeWorld {
    /// Build a world around an already-canonicalized input path, a borrowed
    /// library (from the process-wide cache), and an already-merged font
    /// store. The callers in `TyHtml::compile` / `compileSync` own the
    /// responsibility of building the `FontStore` from the instance's
    /// `base_font_entries` plus any per-call font paths — see
    /// `run_compile_with_world` for the full pipeline.
    fn from_parts(
        input_abs: PathBuf,
        library: &'static LazyHash<Library>,
        fonts: FontStore,
    ) -> Result<Self> {
        let root = input_abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let files = BridgeFiles::new(root, input_abs)?;
        let now = Time::system();

        Ok(Self {
            library,
            fonts,
            files: FileStore::new(files),
            now,
        })
    }
}

impl World for BridgeWorld {
    fn library(&self) -> &LazyHash<Library> {
        self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.files.loader().main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.files.source(id)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.file(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.now.today(offset)
    }
}

// ── Metadata extraction ────────────────────────────────────────────────

fn extract_metadata(document: &HtmlDocument, label_name: &str) -> Option<String> {
    let label = Label::new(PicoStr::intern(label_name))?;
    let content = document.introspector().query_label(label).ok()?;
    let value = content.get_by_name("value").ok()?;
    let json = value_to_json(&value)?;
    serde_json::to_string(&json).ok()
}

fn value_to_json(value: &Value) -> Option<serde_json::Value> {
    match value {
        Value::None => None,
        Value::Bool(b) => Some(serde_json::Value::Bool(*b)),
        Value::Int(i) => Some(serde_json::json!(*i)),
        Value::Float(f) => Some(serde_json::json!(*f)),
        Value::Str(s) => Some(serde_json::json!(s.as_str())),
        Value::Dict(d) => {
            let mut map = serde_json::Map::new();
            for (key, val) in d.iter() {
                if let Some(json_val) = value_to_json(val) {
                    map.insert(key.as_str().to_string(), json_val);
                }
            }
            Some(serde_json::Value::Object(map))
        }
        Value::Array(arr) => {
            let items: Vec<serde_json::Value> = arr.iter().filter_map(value_to_json).collect();
            Some(serde_json::Value::Array(items))
        }
        _ => Some(serde_json::json!(value.repr().as_str())),
    }
}

/// Strip everything outside `<body>…</body>`.
fn extract_body(html: &str) -> &str {
    let start = html
        .find("<body")
        .and_then(|i| html[i..].find('>').map(|j| i + j + 1));
    let end = html.find("</body>");
    match (start, end) {
        (Some(s), Some(e)) if s < e => &html[s..e],
        _ => html,
    }
}

// ── Diagnostic extraction ──────────────────────────────────────────────

/// Build a JS-facing [`CompileDiagnostic`] from a Typst
/// [`SourceDiagnostic`], resolving the span through the world.
///
/// Location fields are populated only when the span resolves to the
/// main input file. Spans in `#import`ed or package files are
/// reported as message-only for now — plumbing paths for those
/// would need either a `path: FileId -> PathBuf` map on
/// `BridgeFiles` or a custom `FileLoader` that records the mapping.
/// Per the roadmap: "Some diagnostics may not have file/span
/// information. The API should allow missing fields."
///
/// We only expose `file` / `line` / `column` (no `range` for now) —
/// that's enough to make build tools log clickable errors and CI logs
/// useful, without the WorldExt::range plumbing.
fn diagnostic_from_source(
    diag: &SourceDiagnostic,
    world: &BridgeWorld,
) -> Option<CompileDiagnostic> {
    let severity = match diag.severity {
        Severity::Warning => CompileDiagnosticSeverity::warning,
        Severity::Error => CompileDiagnosticSeverity::error,
    };

    let file_id = diag.span.id();

    let (file, line, column) = match file_id {
        Some(fid) if fid == world.files.loader().main => {
            // WorldExt::range returns the full byte range; we use
            // `range.start` for the diagnostic location. Detached
            // spans (synthetic diagnostics) return `None` from
            // `range` even if `id()` returns Some, so the inner
            // match handles that case too.
            let source = world.source(fid).ok();
            let byte_offset = world.range(diag.span).map(|r| r.start);
            match (source, byte_offset) {
                (Some(src), Some(off)) => {
                    // `Source::lines().byte_to_line_column()` does
                    // the byte → (line, column) conversion using
                    // typst's own line table — no need to scan the
                    // source ourselves. Both values are 1-indexed.
                    let (l, c) = src.lines().byte_to_line_column(off).unwrap_or((1, 1));
                    (
                        Some(
                            world
                                .files
                                .loader()
                                .main_path
                                .to_string_lossy()
                                .into_owned(),
                        ),
                        Some(l as u32),
                        Some(c as u32),
                    )
                }
                _ => (None, None, None),
            }
        }
        _ => (None, None, None),
    };

    Some(CompileDiagnostic {
        severity,
        message: diag.message.to_string(),
        file,
        line,
        column,
    })
}

/// Long-lived Typst compilation engine.
///
/// `TyHtml` owns the expensive-to-build state (the Typst `Library` and the
/// merged font entry set built from system fonts plus any
/// [`TyHtmlOptions::font_paths`] registered at construction time). Per-call
/// work is reduced to: clone the cached font entries (`Arc` refcount
/// bumps), extend with any per-call [`CompileOptions::font_paths`], build a
/// `BridgeWorld`, and run `typst::compile`.
///
/// Construct once and reuse; the constructor is the explicit cold start.
/// The async [`TyHtml::compile`] moves the blocking work onto a worker
/// thread; [`TyHtml::compileSync`] runs it inline (use only when the
/// caller is itself a sync consumer, e.g. a Vite plugin watch handler).
#[napi]
pub struct TyHtml {
    /// Borrowed from the process-wide `LIBRARY` cache — see module-level docs.
    library: &'static LazyHash<Library>,
    /// System fonts plus any constructor-supplied `font_paths`, materialised
    /// once at construction time. Held behind `Arc` so the async compile
    /// path can move a cheap clone into `spawn_blocking`.
    base_font_entries: Arc<Vec<(CachedFontPath, FontInfo)>>,
}

#[napi]
impl TyHtml {
    /// Build the engine. Cold-start cost: one-shot system-font discovery
    /// (cached for the process) plus a synchronous scan of every
    /// `fontPaths` entry passed here.
    #[napi(constructor)]
    pub fn new(options: Option<TyHtmlOptions>) -> napi::Result<Self> {
        let opts = options.unwrap_or(TyHtmlOptions { font_paths: None });

        // Start from the cached system entries (cheap Arc-bump clone of the
        // slice) and append any constructor-supplied font directories.
        // Each extra directory is scanned exactly once, here, and the
        // results are folded into the instance's base set.
        let mut entries: Vec<(CachedFontPath, FontInfo)> = system_font_entries().to_vec();
        for path in opts.font_paths.unwrap_or_default() {
            let p = PathBuf::from(path);
            for (font_path, info) in typst_kit::fonts::scan(&p) {
                entries.push((CachedFontPath(Arc::new(font_path)), info));
            }
        }

        Ok(Self {
            library: library(),
            base_font_entries: Arc::new(entries),
        })
    }

    /// Compile a Typst source file to HTML and extract metadata.
    ///
    /// Runs on a worker thread so the Node.js event loop is never blocked.
    /// `input` is the path to a `.typ` file on disk; the file's parent
    /// directory becomes the project root for `#import` resolution.
    #[napi]
    pub async fn compile(
        &self,
        input: String,
        options: Option<CompileOptions>,
    ) -> napi::Result<CompileResult> {
        let flat = CompileOptions::flatten_or_default(options);

        let library = self.library;
        let base_entries = Arc::clone(&self.base_font_entries);
        let input_path = PathBuf::from(input);

        napi::tokio::task::spawn_blocking(move || -> Result<CompileResult> {
            let mut fonts = FontStore::new();
            fonts.extend(base_entries.iter().cloned());
            for path in &flat.extra_font_paths {
                fonts.extend(typst_kit::fonts::scan(path));
            }

            let abs = input_path
                .canonicalize()
                .context("Cannot resolve input file path")?;
            let world = BridgeWorld::from_parts(abs, library, fonts)?;
            run_compile_with_world(world, &flat)
        })
        .await
        .map_err(|e| Error::from_reason(format!("worker thread join error: {e}")))?
        .map_err(|e| Error::from_reason(format!("{e:#}")))
    }

    /// Same as [`TyHtml::compile`] but synchronous — runs on the calling
    /// thread and blocks until done. Use this when the caller is itself in
    /// a context where async would race with another sync consumer (e.g.
    /// a Vite plugin watch handler that needs to write its result before
    /// the framework re-evaluates dependent modules).
    ///
    /// **Warning:** this blocks the Node.js event loop for the duration of
    /// the compile (~hundreds of ms). Only call from contexts where that
    /// is acceptable.
    #[napi]
    pub fn compile_sync(
        &self,
        input: String,
        options: Option<CompileOptions>,
    ) -> napi::Result<CompileResult> {
        let flat = CompileOptions::flatten_or_default(options);

        let mut fonts = FontStore::new();
        fonts.extend(self.base_font_entries.iter().cloned());
        for path in &flat.extra_font_paths {
            fonts.extend(typst_kit::fonts::scan(path));
        }

        let input_path = PathBuf::from(input);
        let abs = input_path
            .canonicalize()
            .context("Cannot resolve input file path")
            .map_err(|e| Error::from_reason(format!("{e:#}")))?;
        let world = BridgeWorld::from_parts(abs, self.library, fonts)
            .map_err(|e| Error::from_reason(format!("{e:#}")))?;
        run_compile_with_world(world, &flat).map_err(|e| Error::from_reason(format!("{e:#}")))
    }
}

// ── Compile pipeline ───────────────────────────────────────────────────

/// Run the full compile pipeline against an already-built `BridgeWorld`.
/// Pulled out of the JS entry points so the async / sync methods share
/// the same Typst invocation.
///
/// Diagnostics policy:
///   * All Typst warnings are collected into `diagnostics` (severity:
///     `Warning`) and projected message-only into `warnings` for
///     backwards compat.
///   * Compile errors are also surfaced through `diagnostics`
///     (severity: `Error`) and **the function returns `Ok` with
///     `html = ""`** — the caller is expected to inspect
///     `diagnostics` and react. `run_compile_with_world` only
///     returns `Err` for non-Typst failures (HTML export crash,
///     internal panic). This lets JS consumers distinguish "Typst
///     said no" from "the addon broke".
fn run_compile_with_world(world: BridgeWorld, opts: &FlattenedOptions) -> Result<CompileResult> {
    let Warned { output, warnings } = typst::compile::<HtmlDocument>(&world);

    // Build structured diagnostics for every warning. Filter out the
    // `Comparison` severity (typst's internal incremental annotation)
    // — the helper returns None for those.
    let mut diagnostics: Vec<CompileDiagnostic> = warnings
        .iter()
        .filter_map(|w| diagnostic_from_source(w, &world))
        .collect();

    // Project message-only warnings for the legacy `warnings` field.
    let warnings_msgs: Vec<CompileWarning> = diagnostics
        .iter()
        .filter(|d| matches!(d.severity, CompileDiagnosticSeverity::warning))
        .map(|d| CompileWarning {
            message: d.message.clone(),
        })
        .collect();

    let document = match output {
        Ok(doc) => doc,
        Err(errors) => {
            // Compile halted — surface the errors through diagnostics
            // and bail with an empty HTML rather than throwing. The
            // existing `warnings` field stays empty (errors aren't
            // warnings).
            for e in &errors {
                if let Some(diag) = diagnostic_from_source(e, &world) {
                    diagnostics.push(diag);
                }
            }
            return Ok(CompileResult {
                html: String::new(),
                metadata: None,
                diagnostics,
                warnings: vec![],
            });
        }
    };

    let html_options = HtmlOptions {
        pretty: opts.pretty,
    };
    let html_output = typst_html::html(&document, &html_options)
        .map_err(|e| anyhow::anyhow!("HTML export failed: {:?}", e))?;

    let html_final = if opts.body_only {
        extract_body(&html_output).to_string()
    } else {
        html_output
    };

    let metadata = if opts.no_metadata {
        None
    } else {
        extract_metadata(&document, &opts.label_name)
    };

    Ok(CompileResult {
        html: html_final,
        metadata,
        diagnostics,
        warnings: warnings_msgs,
    })
}
