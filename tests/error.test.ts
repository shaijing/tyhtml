// Smoke test for the error path: a deliberately broken .typ file
// (type error: `let x: int = "not a number"`) should produce an
// empty `html` and at least one diagnostic with severity='error',
// file matching the main fixture, and populated line/column.
//
// Run:  bun tests/error.test.ts
import { fileURLToPath } from 'node:url'
import { createEngine } from './_setup'

const errorFixture = fileURLToPath(new URL('./fixtures/error.typ', import.meta.url))

const engine = createEngine()

console.log('─'.repeat(60))
console.log('Error fixture:')
const errorResult = await engine.compile(errorFixture, { pretty: true })

// Print every diagnostic so a regression shows the actual surface.
for (const d of errorResult.diagnostics) {
    const loc = d.file
        ? `${d.file}:${d.line ?? '?'}:${d.column ?? '?'}`
        : '<no location>'
    console.log(`  [${d.severity}] ${loc}  ${d.message}`)
}
const errorDiags = errorResult.diagnostics.filter((d) => d.severity === 'error')
console.log(`  diagnostics: ${errorResult.diagnostics.length}`)
console.log(`  errors: ${errorDiags.length}`)

// Sanity checks — throw on any failure so the test process exits
// non-zero instead of just logging a problem.
if (errorResult.html !== '') {
    throw new Error(`error fixture: expected html='' but got ${errorResult.html.length} bytes`)
}
if (errorDiags.length === 0) {
    throw new Error('error fixture: expected at least one severity=error diagnostic')
}
const fixtureBasename = errorFixture.split(/[\\/]/).pop()!
const mainError = errorDiags.find(
    (d) => d.file !== undefined && d.file!.endsWith(fixtureBasename),
)
if (!mainError) {
    throw new Error(`error fixture: expected at least one diagnostic with the main fixture path (${fixtureBasename})`)
}
if (mainError.line === undefined || mainError.column === undefined) {
    throw new Error('error fixture: expected main diagnostic to have line/column populated')
}
console.log('  ✓ all error-path assertions passed')
