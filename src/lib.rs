use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use typst::diag::{FileError, FileResult, Warned};
use typst::foundations::{Bytes, Datetime, Duration, Label, Repr, Value};
use typst::introspection::Introspector;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
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
// it on every `compileTypst` call is pure waste.
//
// `typst-kit` font discovery walks `/Library/Fonts`, `/usr/share/fonts`,
// and friends. On macOS the first call can take 200–500 ms; the result is
// effectively immutable for the lifetime of the process, so cache it.
//
// Both caches are bounded to the configured feature set / system font
// set; per-call `fontPaths` are layered on top, see `BridgeWorld::new`.

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

/// Options for `compileTypst`.
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
    /// Additional directories to scan for fonts (can be repeated).
    pub font_paths: Option<Vec<String>>,
}

/// A single Typst warning emitted during compilation.
#[napi(object)]
pub struct CompileWarning {
    pub message: String,
}

/// Result of a successful compilation.
///
/// `metadata` is a JSON-encoded string (e.g. `'{"title":"..."}'`).
/// It is `null` when no `<meta>` label is present in the document,
/// or when `noMetadata: true` is passed. Call `JSON.parse(result.metadata)`
/// on the JS side if you need an object.
#[napi(object)]
pub struct CompileResult {
    pub html: String,
    pub metadata: Option<String>,
    pub warnings: Vec<CompileWarning>,
}

// ── File loader ────────────────────────────────────────────────────────

struct BridgeFiles {
    main: FileId,
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
            PathBuf::from(home).join(".local").join("share").join("typst")
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
        PathBuf::from(home).join("Library").join("Caches").join("typst")
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
    /// Owned per call: cloned from the cached system store (cheap, Arc bumps
    /// + a small `Vec<FontInfo>`) and then extended with any caller-supplied
    /// `fontPaths`.
    fonts: FontStore,
    files: FileStore<BridgeFiles>,
    now: Time,
}

impl BridgeWorld {
    fn new(input_path: &Path, font_paths: &[PathBuf]) -> Result<Self> {
        let abs = input_path
            .canonicalize()
            .context("Cannot resolve input file path")?;
        let root = abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        // Start from the cached system fonts and layer caller-supplied
        // font directories on top. Per-call cost is `O(n_system_fonts)`
        // Arc refcount bumps plus the `FontStore::extend` book rebuild,
        // which is microseconds — much cheaper than re-scanning the
        // system font directories (200–500 ms on macOS).
        let mut fonts = FontStore::new();
        fonts.extend(system_font_entries().iter().cloned());
        for path in font_paths {
            fonts.extend(typst_kit::fonts::scan(path));
        }

        let files = BridgeFiles::new(root, abs)?;
        let now = Time::system();

        Ok(Self {
            library: library(),
            fonts,
            files: FileStore::new(files),
            now,
        })
    }
}

impl World for BridgeWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
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

// ── JS entry point ─────────────────────────────────────────────────────

/// Compile a Typst source file to HTML and extract metadata.
///
/// The compilation runs on a worker thread; this function returns a Promise
/// so the Node.js event loop is never blocked. `input` is the path to a
/// `.typ` file on disk; the file's parent directory becomes the project
/// root for `#import` resolution.
#[napi]
pub async fn compile_typst(
    input: String,
    options: Option<CompileOptions>,
) -> napi::Result<CompileResult> {
    compile_typst_impl(input, options).await
}

/// Same as [`compile_typst`] but synchronous — runs on the calling thread
/// and blocks until done. Use this when the caller is itself in a context
/// where async would race with another sync consumer (e.g. a Vite plugin
/// watch handler that needs to write its result before the framework
/// re-evaluates dependent modules).
///
/// **Warning:** this blocks the Node.js event loop for the duration of
/// the compile (~hundreds of ms). Only call from contexts where that is
/// acceptable.
#[napi]
pub fn compile_typst_sync(
    input: String,
    options: Option<CompileOptions>,
) -> napi::Result<CompileResult> {
    let opts = options.unwrap_or(CompileOptions {
        body_only: None,
        pretty: None,
        no_metadata: None,
        metadata_label: None,
        font_paths: None,
    });

    let body_only = opts.body_only.unwrap_or(false);
    let pretty = opts.pretty.unwrap_or(false);
    let no_metadata = opts.no_metadata.unwrap_or(false);
    let label_name = opts
        .metadata_label
        .unwrap_or_else(|| "meta".to_string());
    let font_paths: Vec<PathBuf> = opts
        .font_paths
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();

    let input_path = PathBuf::from(&input);

    run_compile(input_path, font_paths, body_only, pretty, no_metadata, label_name)
        .map_err(|e| Error::from_reason(format!("{e:#}")))
}

async fn compile_typst_impl(
    input: String,
    options: Option<CompileOptions>,
) -> napi::Result<CompileResult> {
    let opts = options.unwrap_or(CompileOptions {
        body_only: None,
        pretty: None,
        no_metadata: None,
        metadata_label: None,
        font_paths: None,
    });

    let body_only = opts.body_only.unwrap_or(false);
    let pretty = opts.pretty.unwrap_or(false);
    let no_metadata = opts.no_metadata.unwrap_or(false);
    let label_name = opts
        .metadata_label
        .unwrap_or_else(|| "meta".to_string());
    let font_paths: Vec<PathBuf> = opts
        .font_paths
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .collect();

    let input_path = PathBuf::from(&input);

    // Move blocking work onto a worker thread so the event loop stays free.
    let join_result = napi::tokio::task::spawn_blocking(move || {
        run_compile(input_path, font_paths, body_only, pretty, no_metadata, label_name)
    })
    .await
    .map_err(|e| Error::from_reason(format!("worker thread join error: {e}")))?;

    join_result.map_err(|e| Error::from_reason(format!("{e:#}")))
}

fn run_compile(
    input_path: PathBuf,
    font_paths: Vec<PathBuf>,
    body_only: bool,
    pretty: bool,
    no_metadata: bool,
    label_name: String,
) -> Result<CompileResult> {
    let world = BridgeWorld::new(&input_path, &font_paths)?;

    let Warned { output, warnings } = typst::compile::<HtmlDocument>(&world);

    let warnings: Vec<CompileWarning> = warnings
        .iter()
        .map(|w| CompileWarning {
            message: w.message.to_string(),
        })
        .collect();

    let document = output.map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.message.to_string()).collect();
        anyhow::anyhow!("Compilation failed:\n{}", msgs.join("\n"))
    })?;

    let html_options = HtmlOptions { pretty };
    let html_output = typst_html::html(&document, &html_options)
        .map_err(|e| anyhow::anyhow!("HTML export failed: {:?}", e))?;

    let html_final = if body_only {
        extract_body(&html_output).to_string()
    } else {
        html_output
    };

    let metadata = if no_metadata {
        None
    } else {
        extract_metadata(&document, &label_name)
    };

    Ok(CompileResult {
        html: html_final,
        metadata,
        warnings,
    })
}
