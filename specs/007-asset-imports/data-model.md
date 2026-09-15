# Data Model: Asset Imports and Lucide Icons

**Date**: 2026-09-03 · Entities for [spec.md](spec.md).

## IconReference

| Field | Type | Notes |
|-------|------|-------|
| `name` | string | Icon name, case-sensitive |
| `spans` | list | Every referencing source span (file, line, column) |

## IconCollection

Name→bytes map plus per-entry provenance (`package` or `override`). Sorted iteration;
byte-identical bytes for identical inputs.

## AssetSet

Effective sorted bundle asset list: manifest-declared entries plus `assets/icons/<name>`
for each collected icon. Manifest file untouched.

## IconBudget

Documented ceiling: 512 KiB total collected bytes. Shared constant between
implementation and test.
