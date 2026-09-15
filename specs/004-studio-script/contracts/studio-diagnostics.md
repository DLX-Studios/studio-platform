# Contract: Studio Script Diagnostics

**Status**: Contract for feature 004. Additive-only after this document lands; existing
`STUDIO0xx`/`STUDIO2xx` ranges are unchanged.

## Record shape

Every Studio Script diagnostic is one JSON object with the existing shape:

```json
{
  "code": "STUDIO301",
  "severity": "error",
  "message": "browser DOM APIs are outside the portable subset",
  "line": 8,
  "column": 5
}
```

- `code`: stable namespaced code. New codes live in `STUDIO3xx`.
- `severity`: `error` | `warning`.
- `line`/`column`: 1-based source location; always present when the construct has a span.
- No secret material, raw credentials, or host internals in messages.

## STUDIO3xx families

| Code range | Family | When emitted |
|------------|--------|--------------|
| STUDIO300–309 | subset-boundary | Forbidden constructs: browser/Node APIs, `eval`, dynamic import, `any`, reflection, promises/async, exceptions, host calls |
| STUDIO310–319 | types | Untyped/uninferrable bindings, non-serializable values, incompatible slot types, non-Wasm-representable values |
| STUDIO320–329 | runes/backend | Unknown rune, disallowed effect/lifecycle use, construct missing an evaluator or lowering rule |
| STUDIO330–339 | catalog | Unknown component kind, invalid prop/event, unkeyed collection, duplicate node key, bad route/asset reference |
| STUDIO340–349 | graph/swap | Duplicate component, unresolvable import, module cycle (names the cycle), dependency invalidation failure, swap rollback notice |

## Stability

Codes are additive-only; a code never changes meaning. Rejections fail compilation (no IR);
swap-time rejections keep the last valid component running.
