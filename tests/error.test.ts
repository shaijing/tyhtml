import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
import { engine } from './_setup'

const errorFixture = fileURLToPath(new URL('./fixtures/error.typ', import.meta.url))
const importedFixture = fileURLToPath(new URL('./fixtures/imported-main.typ', import.meta.url))

test('compile errors contain an exact one-based source location', async () => {
    const result = await engine.compile(errorFixture, { pretty: true })
    const fixtureBasename = errorFixture.split(/[\\/]/).pop()!
    const diagnostic = result.diagnostics.find(
        (item) => item.severity === 'error' && item.file?.endsWith(fixtureBasename),
    )

    expect(result.html).toBe('')
    expect(diagnostic).toBeDefined()
    expect(diagnostic?.line).toBe(8)
    expect(diagnostic?.column).toBe(7)
})

test('diagnostics resolve locations in imported project files', async () => {
    const result = await engine.compile(importedFixture)
    const diagnostic = result.diagnostics.find(
        (item) => item.severity === 'error' && item.file?.endsWith('imported-error.typ'),
    )

    expect(diagnostic).toBeDefined()
    expect(diagnostic?.line).toBe(1)
    expect(diagnostic?.column).toBe(7)
})
