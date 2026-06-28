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
