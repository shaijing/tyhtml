// Shared test setup. The `TyHtml` cold start (system font discovery +
// Library build) is expensive enough that running it once per test file
// is wasteful; the two test files import `createEngine` from here so the
// whole smoke run only pays for it once.
import { TyHtml } from '../index'

export function createEngine(): TyHtml {
    // Constructor = cold start. We don't pass `fontPaths` here — the
    // smoke tests only exercise the system font set.
    return new TyHtml()
}
