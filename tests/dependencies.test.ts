import { expect, test } from 'bun:test'
import { mkdtempSync, mkdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { engine } from './_setup'

test('exports transitive sources, dynamic data paths, and images in both APIs', async () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), 'tyhtml-deps-')))
    try {
        mkdirSync(join(root, 'parts'))
        writeFileSync(join(root, 'main.typ'), [
            '#include "parts/section.typ"',
            '#let filename = "data" + ".json"',
            '#json(filename).title',
            '#read("data.json")',
            '#image("logo.svg")',
            '#if false { read("unused.txt") }',
        ].join('\n'))
        writeFileSync(join(root, 'parts/section.typ'), '#include "nested.typ"')
        writeFileSync(join(root, 'parts/nested.typ'), 'Nested content')
        writeFileSync(join(root, 'data.json'), '{"title":"Resource title"}')
        writeFileSync(join(root, 'logo.svg'),
            '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10"/></svg>')
        const input = join(root, 'main.typ')
        const result = await engine.compile(input, { bodyOnly: true, noMetadata: true })
        expect(result.diagnostics.filter(d => d.severity === 'error')).toEqual([])
        expect(result.dependencies).toEqual([
            'main.typ', 'parts/section.typ', 'parts/nested.typ', 'data.json', 'logo.svg',
        ].map(path => join(root, path)).sort())
        expect(engine.compileSync(input).dependencies).toEqual(result.dependencies)

        // A fresh world must observe resource edits and drop stale dependencies.
        writeFileSync(join(root, 'data.json'), '{"title":"Updated title"}')
        expect((await engine.compile(input)).html).toContain('Updated title')
        writeFileSync(input, 'No resources')
        expect((await engine.compile(input)).dependencies).toEqual([input])
    } finally {
        rmSync(root, { recursive: true, force: true })
    }
})

test('failed reads remain watchable and compilation recovers after file creation', async () => {
    const root = realpathSync(mkdtempSync(join(tmpdir(), 'tyhtml-missing-')))
    try {
        const input = join(root, 'main.typ')
        const missing = join(root, 'missing.txt')
        writeFileSync(input, '#read("missing.txt")')
        const result = await engine.compile(input)
        expect(result.html).toBe('')
        expect(result.diagnostics.some(d => d.severity === 'error')).toBeTrue()
        expect(result.dependencies).toEqual([input, missing].sort())
        expect(engine.compileSync(input).dependencies).toEqual(result.dependencies)
        writeFileSync(missing, 'Created resource')
        const recovered = await engine.compile(input)
        expect(recovered.html).toContain('Created resource')
        expect(recovered.dependencies).toEqual(result.dependencies)
    } finally {
        rmSync(root, { recursive: true, force: true })
    }
})
