# CLAUDE.md

Project-specific instructions for Claude Code.

All detailed guidance for working on this codebase lives in [`AGENTS.md`](./AGENTS.md). This file exists to import it via Claude Code's `@` syntax — do not duplicate content here.

@AGENTS.md

## Quick reminders

- Bun is the preferred package manager (`package.json#packageManager` is pinned).
- `napi build` regenerates `index.js`, `index.d.ts`, and the `.node` binary — all gitignored.
- The JS surface is a single class: `TyHtml`. `new TyHtml()` is the explicit cold start (Library + system fonts + constructor `fontPaths`). `engine.compile` is async (worker thread); `engine.compileSync` only when async would race with another sync consumer.
- Platform matrix is the four triples in `package.json#napi.targets`. Adding one means: targets + optionalDependencies + build script + README row + AGENTS.md §4 row.
- Before committing, run `bun tests/test.ts` and `bunx tsc --noEmit`.