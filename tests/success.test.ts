// Smoke test for the success path: hello.typ compiles cleanly to HTML,
// exposes a metadata label, and produces one warning (the synthetic
// "html export is under active development" diagnostic from typst itself).
//
// Run:  bun tests/success.test.ts
import { fileURLToPath } from 'node:url'
import { createEngine } from './_setup'

const fixture = fileURLToPath(new URL('./fixtures/hello.typ', import.meta.url))

const engine = createEngine()

const result = await engine.compile(fixture, {
    bodyOnly: false,
    pretty: true,
})

console.log('─'.repeat(60))
console.log('HTML (first 400 chars):')
console.log(result.html.slice(0, 400))
console.log('─'.repeat(60))
console.log('Metadata:')
console.log(result.metadata)
if (result.metadata) console.log(JSON.parse(result.metadata))
console.log('─'.repeat(60))
console.log(`Diagnostics: ${result.diagnostics.length}`)
for (const d of result.diagnostics) {
    const loc = d.file
        ? `${d.file}:${d.line ?? '?'}:${d.column ?? '?'}`
        : '<no location>'
    console.log(`  [${d.severity}] ${loc}  ${d.message}`)
}
// `warnings` is the message-only projection of severity=warning entries
// from `diagnostics`. Kept here for backwards-compat coverage.
console.log(`Warnings (legacy field): ${result.warnings.length}`)
if (result.warnings.length > 0) {
    for (const w of result.warnings) console.log(`  - ${w.message}`)
}

// Exercise compileSync on the same instance to confirm the cache is
// shared across both entry points.
const syncResult = engine.compileSync(fixture, { pretty: true })
console.log('─'.repeat(60))
console.log(`compileSync html length matches compile: ${syncResult.html.length === result.html.length}`)
console.log(`compileSync diagnostics matches compile: ${syncResult.diagnostics.length === result.diagnostics.length}`)
