# tyhtml

Native Node.js addon (Rust + [napi-rs](https://napi.rs)) that compiles [Typst](https://typst.app) `.typ` files to HTML and extracts metadata.

## Installation

```bash
npm install tyhtml
```

The package ships with prebuilt binaries for the following platforms via npm `optionalDependencies`:

| Platform | Package |
|---|---|
| Windows x64 | `tyhtml-win32-x64-msvc` |
| Linux x64 (glibc) | `tyhtml-linux-x64-gnu` |

If your platform is not in this list, `npm install` will succeed (the binaries are `optionalDependencies`) but importing the module will fail at runtime — you'll need to build from source.

## Usage

```ts
import { compileTypst } from 'tyhtml'

const result = await compileTypst('path/to/file.typ', {
  pretty: true,                  // pretty-print the HTML output
  bodyOnly: false,               // false = full <!DOCTYPE>...<body>; true = strip wrapper
  noMetadata: false,             // set true to skip the <meta> label query (faster)
  metadataLabel: 'meta',         // override the default label queried for metadata
  fontPaths: ['C:/extra/fonts'], // additional font directories
})

console.log(result.html)
// → '<!DOCTYPE html><html>...'

const meta = result.metadata ? JSON.parse(result.metadata) : null
console.log(meta)
// → { title: 'Hello', tags: ['a', 'b'], ... }
```

The full API surface is in [`index.d.ts`](./index.d.ts) (auto-generated from `src/lib.rs`).

## Building from source

Requires:

- Rust toolchain (edition 2024)
- Node.js ≥ 14
- For the Linux x64 cross-build: [zig](https://ziglang.org/) ≥ 0.13 and `@napi-rs/cross-toolchain` (`npm i -D @napi-rs/cross-toolchain`)

```bash
# Install JS deps
npm install

# Build for the current host platform
npm run build

# Build for both supported target platforms
# (host + Linux x64 via cargo-zigbuild)
npm run build:all
```

## Tests

```bash
bun tests/test.ts
# → compiles tests/fixtures/hello.typ and prints the HTML + metadata
```

## Publishing

```bash
# 1. Build for both platforms and scaffold the npm/ scoped sub-packages
npm run prepublishOnly

# 2. Login (one-time)
npm login

# 3. Publish root + each scoped sub-package
npx napi pre-publish
```

`napi pre-publish` iterates over every target in `napi.targets`, publishes the corresponding `tyhtml-{triple}` package, then publishes the root package which lists them all under `optionalDependencies`. Consumers get the right binary for their platform automatically.

## License

MIT — see [LICENSE](./LICENSE).
