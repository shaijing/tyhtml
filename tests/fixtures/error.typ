// This fixture is deliberately broken: the integer variable `x` is
// assigned a string value, which is a hard type error in Typst.
// Used by tests/test.ts to verify the error path — that compile
// errors surface through `result.diagnostics` with severity 'error',
// populated file / line / column, and that `result.html` is the
// empty string.

#let x: int = "not a number"
