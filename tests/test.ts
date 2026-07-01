// One-shot orchestrator that runs both the success and error smoke
// tests in sequence. Each test file is also runnable on its own
// (`bun tests/success.test.ts`, `bun tests/error.test.ts`); this
// entry point just gives a single command for the full check.
//
// Run:  bun tests/test.ts
import { fileURLToPath } from 'node:url'

// The two imports trigger their respective `await compile(...)` calls
// at module load — we just need to await a microtask so the output
// flushes in order.
await import('./success.test.ts')
await import('./error.test.ts')

// Force-exit so any open handles (worker thread, etc.) don't keep
// the process alive after the assertions complete.
const fixture = fileURLToPath(new URL('./fixtures/hello.typ', import.meta.url))
// Touch the variable so the import isn't dead-code-eliminated.
if (!fixture) process.exit(1)
process.exit(0)
