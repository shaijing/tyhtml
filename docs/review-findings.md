# Code Review Findings

Date: 2026-06-28

This file captures review findings for handoff to another agent. The review was based on the current working tree changes around the Rust N-API Typst HTML compiler.

## Findings

### 1. Metadata default label is inconsistent

Severity: Medium

Locations:

- `src/lib.rs:301`
- `index.d.ts:11`
- `src/lib.rs:30`
- `src/lib.rs:45`

Issue:

The implementation defaults `metadata_label` to `"meta"`:

```rust
let label_name = opts
    .metadata_label
    .unwrap_or_else(|| "meta".to_string());
```

But the public type docs and Rust comments say the default is `"post_info"`. A user following the docs and writing `#metadata(...) <post_info>` will get `metadata: null` unless they pass `metadataLabel: "post_info"`.

Suggested fix:

Choose one default and make implementation plus docs match. Based on the current fixture, `"meta"` appears intentional because `tests/fixtures/hello.typ` uses `#metadata(post_info) <meta>`. If that is the desired contract, update the comments in `src/lib.rs` and `index.d.ts`; otherwise change the implementation default to `"post_info"` and update the fixture.

### 2. `fibonacci` remains in JS/types/docs after being removed from Rust source

Severity: Medium

Locations:

- `src/lib.rs:285`
- `index.js:593`
- `index.d.ts:50`
- `README.md:24`
- `README.md:55`

Issue:

The current Rust source only exports `compile_typst`, but generated/public files still expose `fibonacci`. Current local runtime still reports `fibonacci` as a function because the existing `.node` binary appears to be stale or built from older code. After regenerating artifacts from the current Rust source, `fibonacci` will likely disappear while `index.js`, `index.d.ts`, and README still reference it.

Suggested fix:

Regenerate N-API artifacts from the current Rust source, then remove `fibonacci` from `index.js`, `index.d.ts`, and README. Alternatively, restore the Rust `fibonacci` export if it must remain part of the public API. Update README usage and tests to demonstrate `compileTypst`.

### 3. Published platform docs do not match package configuration

Severity: Medium

Locations:

- `package.json:27`
- `package.json:33`
- `package.json:44`
- `README.md:13`

Issue:

README says prebuilt packages are shipped for Windows x64, Linux x64 GNU, macOS x64, and macOS arm64. The actual `package.json` only lists optional dependencies and N-API targets for:

- `tyhtml-win32-x64-msvc`
- `tyhtml-linux-x64-gnu`

Also, `build:all` only runs the current platform build plus Linux x64 GNU. macOS users may reasonably expect a prebuilt binary to install, but importing the package would fail if no macOS optional package is published.

Suggested fix:

Either add the macOS targets and optional dependencies to `package.json` and update `build:all`, or change README to list only the actually published targets. Also note that the README package naming style (`tyhtml-x86_64-pc-windows-msvc`) does not match the loader/package style currently used by `index.js` (`tyhtml-win32-x64-msvc`).

## Verification Run

The following commands completed successfully during review:

```bash
cargo test
cargo clippy --all-targets --all-features
bun tests/test.ts
bunx tsc --noEmit
```

Notes:

- `cargo test` passed but reported `0 tests`.
- `bun tests/test.ts` is currently a smoke test and successfully compiled `tests/fixtures/hello.typ`.
- The smoke test produced one Typst warning: `html export is under active development and incomplete`.

---

Date: 2026-06-29

This review covered the current main branch after the TyHtml class API refactor and the release profile / benchmark commit. The working tree was clean at review time.

## Findings

### 1. Rust formatting check fails

Severity: Medium

Locations:

- build.rs:5
- src/lib.rs:230
- src/lib.rs:246
- src/lib.rs:415
- src/lib.rs:495
- src/lib.rs:520

Issue:

cargo fmt --check reports formatting diffs. This is not a runtime bug, but it will fail any CI or release gate that enforces rustfmt. The reported diff includes indentation in build.rs and several line-wrapping changes in src/lib.rs.

Suggested fix:

Run cargo fmt and commit the formatting-only changes.

### 2. Clippy warnings remain after the refactor

Severity: Low

Locations:

- src/lib.rs:269
- src/lib.rs:306

Issue:

cargo clippy --all-targets --all-features completes, but emits two warnings:

- clippy::doc_lazy_continuation on the fontPaths doc comment.
- clippy::needless_borrow in World::library, where &self.library is immediately dereferenced by the compiler.

Suggested fix:

Indent the continuation doc line, and return self.library directly from World::library.

### 3. Agent guidance still describes the old release profile

Severity: Low

Locations:

- Cargo.toml:24
- AGENTS.md:44

Issue:

Cargo.toml now uses opt-level = 3, but AGENTS.md still describes the release profile as size-optimized with opt-level set to z. This can mislead future maintainers or coding agents about the intended binary-size/performance trade-off.

Suggested fix:

Update AGENTS.md to describe the current speed-optimized release profile, including the known binary-size trade-off if that context should remain visible.

### 4. Fixture text still mentions the removed top-level API

Severity: Nit

Location:

- tests/fixtures/hello.typ:3

Issue:

The fixture body says it is for compileTypst, but the public JS API is now the TyHtml class. This does not affect test execution, but the smoke test prints generated HTML from this fixture, so old API terminology can still appear in review output.

Suggested fix:

Change the fixture sentence to reference TyHtml or the smoke test generically.

## Verification Run

The following commands completed successfully during review:

    cargo test
    bunx tsc --noEmit
    bun tests/test.ts
    bun bench/run.ts 2

The following commands did not pass cleanly:

    cargo fmt --check
    cargo clippy --all-targets --all-features

Notes:

- cargo test passed but reported 0 tests.
- bun tests/test.ts successfully exercised both engine.compile and engine.compileSync.
- bun bench/run.ts 2 completed and reported timing stats.
- cargo fmt --check failed due to rustfmt diffs.
- cargo clippy --all-targets --all-features compiled successfully but emitted two warnings.
