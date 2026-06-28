# tyhtml

Native Node.js addon (Rust + [napi-rs](https://napi.rs)) that exposes HTML processing primitives to JavaScript.

## Installation

```bash
npm install tyhtml
```

The package ships with prebuilt binaries for the following platforms via npm `optionalDependencies`:

| Platform | Triple | Package |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `tyhtml-x86_64-pc-windows-msvc` |
| Linux x64 (glibc) | `x86_64-unknown-linux-gnu` | `tyhtml-x86_64-unknown-linux-gnu` |
| macOS x64 | `x86_64-apple-darwin` | `tyhtml-x86_64-apple-darwin` |
| macOS arm64 (Apple Silicon) | `aarch64-apple-darwin` | `tyhtml-aarch64-apple-darwin` |

If your platform is not in this list, `npm install` will succeed (the binaries are `optionalDependencies`) but importing the module will fail at runtime — you'll need to build from source.

## Usage

```ts
import { fibonacci } from 'tyhtml'

console.log(fibonacci(10))   // 55
```

The full API surface is in [`index.d.ts`](./index.d.ts) (auto-generated from `src/lib.rs`).

## Building from source

Requires:

- Rust toolchain (edition 2024)
- Node.js ≥ 14
- For cross-platform builds: [zig](https://ziglang.org/) ≥ 0.13 and `@napi-rs/cross-toolchain` (`npm i -D @napi-rs/cross-toolchain`)

```bash
# Install JS deps
npm install

# Build for the current host platform
npm run build

# Build for all 4 target platforms (cross-compile via cargo-zigbuild)
npm run build:all
```

> **Note**: Cross-compiling to `darwin-*` targets from a non-macOS host additionally requires an macOS SDK (e.g. via [`osxcross`](https://github.com/tpoechtrager/osxcross)). The Linux and Windows targets work out of the box on any host with zig.

## Tests

```bash
bun tests/test.ts
# → Fibonacci of 10 is: 55
```

## Publishing

```bash
# 1. Build for all platforms and scaffold the npm/ scoped sub-packages
npm run prepublishOnly

# 2. Dry-run check
npx napi pre-publish --dry-run

# 3. Login (one-time)
npm login

# 4. Publish root + each scoped sub-package
npx napi pre-publish
```

`napi pre-publish` iterates over every target, publishes the corresponding `tyhtml-{triple}` package, then publishes the root package which lists them all under `optionalDependencies`. Consumers get the right binary for their platform automatically.

## License

MIT — see [LICENSE](./LICENSE).
