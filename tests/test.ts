import { TyHtml } from '../index'
import { fileURLToPath } from 'node:url'

const fixture = fileURLToPath(new URL('./fixtures/hello.typ', import.meta.url))
const errorFixture = fileURLToPath(new URL('./fixtures/error.typ', import.meta.url))

// Cold start is the constructor — system-font discovery and the optional
// base fontPaths scan happen here, once.
const engine = new TyHtml()

// ── Success path ────────────────────────────────────────────────────
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

// ── Error path ─────────────────────────────────────────────────────
console.log('─'.repeat(60))
console.log('Error fixture:')
const errorResult = await engine.compile(errorFixture, { pretty: true })
console.log(`  html is empty: ${errorResult.html === ''}`)
console.log(`  diagnostics: ${errorResult.diagnostics.length}`)
for (const d of errorResult.diagnostics) {
    const loc = d.file
        ? `${d.file}:${d.line ?? '?'}:${d.column ?? '?'}`
        : '<no location>'
    console.log(`  [${d.severity}] ${loc}  ${d.message}`)
}
const errorDiags = errorResult.diagnostics.filter((d) => d.severity === 'error')
console.log(`  errors: ${errorDiags.length}`)
// Sanity checks — actual assertion logic (so a regression trips the
// process exit code, not just a log line).
if (errorResult.html !== '') {
    throw new Error(`error fixture: expected html='' but got ${errorResult.html.length} bytes`)
}
if (errorDiags.length === 0) {
    throw new Error('error fixture: expected at least one severity=error diagnostic')
}
const mainError = errorDiags.find(
    (d) => d.file !== undefined && errorFixture.endsWith(d.file!.split(/[\\/]/).pop()!),
)
if (!mainError) {
    throw new Error('error fixture: expected at least one diagnostic with the main fixture path')
}
if (mainError.line === undefined || mainError.column === undefined) {
    throw new Error('error fixture: expected main diagnostic to have line/column populated')
}
console.log('  ✓ all error-path assertions passed')