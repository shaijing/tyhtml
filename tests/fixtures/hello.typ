= Hello from typst

This is a *minimal* fixture for the `TyHtml` engine.

#let post_info = (
  title: "Hello from typst",
  date: "2026-06-28",
  tags: ("test", "napi-rs", "tyhtml"),
  draft: false,
)

#metadata(post_info) <meta>

= Section
A paragraph with #link("https://example.com")[a link].

= Math
$ E = m c^2 $