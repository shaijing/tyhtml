import { TyHtml } from '../index'
import { fileURLToPath } from 'node:url'

const fixture = fileURLToPath(new URL('./fixtures/hello.typ', import.meta.url))

// Cold start is the constructor — system-font discovery and the optional
// base fontPaths scan happen here, once.
const engine = new TyHtml()

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
console.log(`Warnings: ${result.warnings.length}`)
if (result.warnings.length > 0) {
  for (const w of result.warnings) console.log(`  - ${w.message}`)
}

// Exercise compileSync on the same instance to confirm the cache is
// shared across both entry points.
const syncResult = engine.compileSync(fixture, { pretty: true })
console.log('─'.repeat(60))
console.log(`compileSync html length matches compile: ${syncResult.html.length === result.html.length}`)