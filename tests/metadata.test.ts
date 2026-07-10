import { expect, test } from 'bun:test'
import { fileURLToPath } from 'node:url'
import { engine } from './_setup'

const noneFixture = fileURLToPath(new URL('./fixtures/metadata-none.typ', import.meta.url))
const duplicateFixture = fileURLToPath(
    new URL('./fixtures/metadata-duplicate.typ', import.meta.url),
)

test('metadata preserves nested none values as JSON null', async () => {
    const result = await engine.compile(noneFixture)

    expect(JSON.parse(result.metadata!)).toEqual({
        items: [1, null, 2],
        missing: null,
    })
})

test('duplicate metadata labels fail explicitly', async () => {
    await expect(engine.compile(duplicateFixture)).rejects.toThrow(
        'Metadata label "meta" occurs 2 times',
    )
})
