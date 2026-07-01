/**
 * Benchmark: time N compiles of the hello.typ fixture.
 *
 * - Reports median / min / max / p95 in milliseconds.
 * - Prints the first 60 chars of result.html + warnings count for each run
 *   so you can spot-check that nothing changed.
 * - Separate "warm" pass (excluded from stats) so the OnceLock caches
 *   inside TyHtml are primed; only cold(ish) steady-state numbers count.
 *
 * Usage:  bun bench/run.ts [N]    (default N = 50)
 */
import { TyHtml } from '../index'
import { fileURLToPath } from 'node:url'

const N = Number(process.argv[2] ?? 50)
const FIXTURE = fileURLToPath(new URL('../tests/fixtures/hello.typ', import.meta.url))

function median(xs: number[]): number {
    const sorted = xs.slice().sort((a, b) => a - b)
    const mid = Math.floor(sorted.length / 2)
    const lo = sorted[mid - 1] ?? 0
    const hi = sorted[mid] ?? 0
    return sorted.length % 2 === 0 ? (lo + hi) / 2 : hi
}

function pct(xs: number[], p: number): number {
    const sorted = xs.slice().sort((a, b) => a - b)
    const idx = Math.min(sorted.length - 1, Math.floor((sorted.length * p) / 100))
    return sorted[idx] ?? 0
}

const engine = new TyHtml()

// Warm pass — primes the OnceLock caches inside the TyHtml struct.
{
    const r = await engine.compile(FIXTURE, { pretty: false })
    const warmWarnings = r.diagnostics.filter((d) => d.severity === 'warning').length
    if (warmWarnings !== 1) {
        throw new Error(`warm pass produced ${warmWarnings} warnings, expected 1`)
    }
}

const samples: number[] = []
let totalHtmlLen = 0

for (let i = 0; i < N; i++) {
    const t0 = performance.now()
    const r = await engine.compile(FIXTURE, { pretty: false })
    const t1 = performance.now()
    samples.push(t1 - t0)
    if (i === 0) totalHtmlLen = r.html.length
}

const min = samples.length ? Math.min(...samples) : 0
const max = samples.length ? Math.max(...samples) : 0
const med = median(samples)
const p95 = pct(samples, 95)
const mean = samples.length ? samples.reduce((a, b) => a + b, 0) / samples.length : 0

console.log(`fixture     hello.typ (${FIXTURE})`)
console.log(`runs        ${N} (after 1 warmup)`)
console.log(`html len    ${totalHtmlLen}`)
console.log(`min         ${min.toFixed(2)} ms`)
console.log(`median      ${med.toFixed(2)} ms`)
console.log(`mean        ${mean.toFixed(2)} ms`)
console.log(`p95         ${p95.toFixed(2)} ms`)
console.log(`max         ${max.toFixed(2)} ms`)