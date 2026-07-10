import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
import { engine } from './_setup'

const fixture = fileURLToPath(new URL('./fixtures/hello.typ', import.meta.url))

test('compile returns HTML, metadata, and structured warnings', async () => {
    const result = await engine.compile(fixture, { pretty: true })

    expect(result.html).toStartWith('<!DOCTYPE html>')
    expect(JSON.parse(result.metadata!)).toEqual({
        date: '2026-06-28',
        draft: false,
        tags: ['test', 'napi-rs', 'tyhtml'],
        title: 'Hello from typst',
    })
    expect(result.diagnostics.some((d) => d.severity === 'warning')).toBeTrue()
})

test('compileSync matches the async result', async () => {
    const asyncResult = await engine.compile(fixture, { pretty: true })
    const syncResult = engine.compileSync(fixture, { pretty: true })

    expect(syncResult).toEqual(asyncResult)
})

test('bodyOnly and noMetadata honor their output contracts', async () => {
    const result = await engine.compile(fixture, {
        bodyOnly: true,
        noMetadata: true,
    })

    expect(result.html).not.toContain('<!DOCTYPE')
    expect(result.html).not.toContain('<body')
    expect(result.html).toContain('Hello from typst')
    expect(Object.hasOwn(result, 'metadata')).toBeFalse()
    expect(result.metadata).toBeUndefined()
})
