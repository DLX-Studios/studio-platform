# Upstream provenance

- Project: `https://github.com/baseballyama/rsvelte`
- Revision: `b64aed9c611557649ebd6428455cfe874255356e` (main, 2026-09-03)
- Upstream package versions: `rsvelte 0.11.1`, `rsvelte_core 0.11.1`, `rsvelte_esrap 0.10.37`
- Editor crates (feature 008): `rsvelte_fmt`, `rsvelte_formatter`, `tailwind_class_order`,
  `rsvelte_lint`, `rsvelte_diagnostics`, `rsvelte_preprocess` (manifest-only),
  `rsvelte_projection`, same pinned rev, same manifest-only lint deltas
- License: MIT (see `LICENSE`)

Studio vendors three upstream crates — the `rsvelte` compiler facade, its `rsvelte_core`
parse/analysis core, and `rsvelte_esrap` — preserving their relative layout so the workspace
`path = "../..."` dependencies keep resolving. Only `studio-script/src/rsvelte_adapter.rs` may
depend on these crates; no `rsvelte` type crosses the `studio-script` boundary, and the
forbidden `compile()`/`compile_both()` code-generation entry points are never called.

A Cargo git dependency was rejected: `cargo fetch` fails resolving a nested fixture
submodule (`src-tauri/lib/portable-vocal-remover`, `git@github.com:` SSH URL), so the
checkout cannot be reproduced by Cargo alone. Vendoring keeps builds offline and
reproducible; the MIT copyright notice ships in `LICENSE` per the license terms.

## Studio deltas

Sources are byte-identical to the upstream revision listed above. Build-system integration
requires two manifest-only deltas (no `.rs` file is modified):

1. `[lints] workspace = true` in each vendored `Cargo.toml` is replaced with the upstream
   workspace's own lint table (`unsafe_op_in_unsafe_fn`, `undocumented_unsafe_blocks`,
   `format_push_string` denied; `too_many_arguments`, `type_complexity` allowed). Without this,
   Cargo resolves `workspace = true` against Studio's table (`unsafe_code = forbid`), which the
   upstream string-writer legitimately violates.
2. Upstream `tests/`, `benches/`, `demo/`, and `examples/` directories are pruned
   (they are excluded from the workspace and never compile), along with their
   `[[bench]]` manifest stanzas (they are excluded from the workspace and never built).
3. Vendored crates `rsvelte_ast_equiv` (dev-dependency of `rsvelte_core`) and
   `rsvelte_projection` (optional dependency of the facade) are included so manifests resolve,
   even though Studio enables neither.

Upstream's `oxc` git `[patch]` is intentionally not copied: Studio never names `oxc` types
directly, so crates.io `oxc 0.146` needs no unification.

Compatibility fixtures in `crates/studio-script/tests/` pin the Svelte shapes Studio relies on;
an upstream update is accepted only after those fixtures pass against the new revision.
