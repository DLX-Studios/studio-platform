# Studio Script fixture corpus

The parser-of-record tests and the `studio check`/`studio fmt` commands use
these small, reviewable `.studio` documents as a shared corpus.

- `valid/` contains canonical and intentionally non-canonical but valid input
  covering scripts, comments, nested nodes, text, bindings, and token refs.
- `invalid/` contains stable-ID, version, syntax, expression, and hostile
  nesting failures. Every invalid document must produce at least one stable
  `STUDIO###` diagnostic rather than panic or hang.

The canonical v1 header is `studio 1`. Script blocks use
`<script lang="studio">`; `studio` is the Studio Script language label and is
not a TypeScript source block.
