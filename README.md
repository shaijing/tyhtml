# TyHtml

Native Node.js addon (Rust + [napi-rs](https://napi.rs)) that compiles [Typst](https://typst.app) `.typ` files to HTML and extracts metadata.

## Installation

[Bun](https://bun.sh) is the preferred package manager for this project (the repo ships a `bun.lock` and the test suite runs on Bun):

```bash
bun add @isomtop/tyhtml
```

Other package managers work too — Bun just resolves the platform-specific `optionalDependencies` faster and the native binary loads via Bun's N-API shim out of the box:

```bash
npm install @isomtop/tyhtml
# or
pnpm add @isomtop/tyhtml
# or
yarn add @isomtop/tyhtml
```

The package ships with prebuilt binaries for the following platforms via npm `optionalDependencies`:

| Platform | Package |
|---|---|
| Windows x64 (MSVC) | `@isomtop/tyhtml-win32-x64-msvc` |
| Linux x64 (glibc) | `@isomtop/tyhtml-linux-x64-gnu` |
| macOS Apple Silicon (arm64) | `@isomtop/tyhtml-darwin-arm64` |
| macOS Intel (x64) | `@isomtop/tyhtml-darwin-x64` |

The macOS binaries are **universal in the sense of API**, but each architecture ships as its own npm package — `npm install` will pick the right one for the host automatically. If your platform is not in this list, `npm install` will succeed (the binaries are `optionalDependencies`) but importing the module will fail at runtime — you'll need to build from source.

## Usage

The native addon exports a single class, `TyHtml`. Construct once (this is the explicit cold start — system-font discovery plus any constructor `fontPaths` scan happen here), then call `compile` / `compileSync` as many times as you like.

```ts
import { TyHtml } from '@isomtop/tyhtml'

// Constructor = cold start. Pass base fontPaths here if you need them.
const engine = new TyHtml({
  fontPaths: ['C:/extra/fonts'],  // scanned once at construction
})

// Async — runs on a worker thread, never blocks the event loop.
const result = await engine.compile('path/to/file.typ', {
  pretty: true,                  // pretty-print the HTML output
  bodyOnly: false,               // false = full <!DOCTYPE>...<body>; true = strip wrapper
  noMetadata: false,             // set true to skip the <meta> label query (faster)
  metadataLabel: 'meta',         // override the default label queried for metadata
  fontPaths: ['/tmp/extra'],     // per-call extras, layered on top of constructor set
})

console.log(result.html)
// → '<!DOCTYPE html><html>...'

const meta = result.metadata ? JSON.parse(result.metadata) : null
console.log(meta)
// → { title: 'Hello', tags: ['a', 'b'], ... }

// Sync variant — same instance, same caches, runs inline on the caller.
// Use in contexts where async would race with another sync consumer
// (e.g. a Vite plugin watch handler).
const syncResult = engine.compileSync('path/to/file.typ', { pretty: true })
```

The full API surface is in [`index.d.ts`](./index.d.ts) (auto-generated from `src/lib.rs`).

## Building from source

Requires:

- Rust toolchain (edition 2024)
- Node.js ≥ 14
- For the Linux x64 cross-build: [zig](https://ziglang.org/) ≥ 0.13 and `@napi-rs/cross-toolchain` (`npm i -D @napi-rs/cross-toolchain`)
- For the macOS (Darwin) cross-builds: an Apple SDK — easiest path is to run the host build on macOS (`npm run build` produces a darwin binary for the current arch), or set up the `osxcross` toolchain referenced by `@napi-rs/cross-toolchain`

```bash
# Install JS deps
npm install

# Build for the current host platform
npm run build

# Build for every supported target (host + Linux x64 + Darwin arm64 + Darwin x64)
npm run build:all

# Or build a single target explicitly:
npm run build:win32-x64-msvc
npm run build:linux-x64-gnu
npm run build:darwin-arm64
npm run build:darwin-x64
```

## Tests

```bash
bun tests/test.ts
# → compiles tests/fixtures/hello.typ and prints the HTML + metadata
```

## Publishing

```bash
# 1. Build for every supported target and scaffold the npm/ scoped sub-packages
npm run prepublishOnly

# 2. Login (one-time)
npm login

# 3. Publish root + each scoped sub-package
npx napi pre-publish
```

`napi pre-publish` iterates over every target in `napi.targets` (Windows x64, Linux x64, Darwin arm64, Darwin x64), publishes the corresponding `@isomtop/tyhtml-{triple}` package, then publishes the root package which lists them all under `optionalDependencies`. Consumers get the right binary for their platform automatically.

## License

MIT — see [LICENSE](./LICENSE).
