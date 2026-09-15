# 31 [R]: Semantic renderer expansion — containers, text, media

**What to build:** Every catalog kind in the container/text/media family gains a semantically complete native renderer honoring its full property schema, replacing generic fallback surfaces. The component readiness matrix advances per kind.

**Blocked by:** None (can start immediately)

**Status:** complete (source/test evidence; visual runtime fixture remains external)

- [x] Each kind in batch renders all declared properties correctly (`crates/studio-app/src/foundation.rs:1253`, `:2587`, `:1636`)
- [x] Targeted patches update visuals without remounting subtrees (`crates/studio-app/src/foundation.rs:1229`, `:2763`)
- [x] Matrix records rendered/verified states per kind (`crates/studio-components/src/catalog.rs:301`, `docs/component-matrix.md:5`)
- [ ] Visual regression fixtures captured for stable overlays and frames (external visual-fixture gate)

## Implementation Notes

- Added the shared renderer readiness contract in `studio-components` and the durable
  `docs/component-matrix.md` matrix. Batch A container/text/media/display branches are backed by
  the native GPUI dispatch and the matrix records their certified state.
- Expanded the GPUI `plugin_node` Batch A branches for stable retained IDs, common
  `visible`/`opacity`/`accessibility_label` handling, semantic container/text/media properties,
  and non-WebP asset format detection while preserving existing `*-img` and `*-cart-img` sizing.
- Added in-code Batch A visual/state fixture coverage alongside readiness metadata and image format
  detection tests (`crates/studio-app/src/foundation.rs:3190`). Focused renderer tests and format
  checks pass; compositor screenshots remain an external gate.
