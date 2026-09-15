# Data Model: File-Based Routing

**Date**: 2026-09-03 · Entities for [spec.md](spec.md).

## RouteEntry

| Field | Type | Notes |
|-------|------|-------|
| `path` | string | Pattern, e.g. `/orders/:id` |
| `segments` | list | Static text, `:param`, or trailing `*` |
| `params` | list of string | Parameter names in order |
| `title` | string | From `title` export or default |
| `file` | string | Project-relative source path |
| `kind` | enum | `static` \| `param` \| `wildcard` |

Validation rules: unique paths; unique shapes (param names must agree per shape);
well-formed segments; wildcard last only.

## RouteRegistry

Sorted entries plus optional not-found entry. Deterministic render; byte-identical for
identical inputs.

## RouteMatch

Entry plus extracted `{param: value}` map, or an explicit miss.

## RouteDiagnostic

Stable code (`ROUTE_DUPLICATE_*`, `ROUTE_AMBIGUOUS_*`, `ROUTE_MALFORMED_*`,
`ROUTE_MISMATCH_*`), message, implicated files.
