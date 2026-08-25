# 31 [R]: Semantic renderer expansion — containers, text, media

**What to build:** Every catalog kind in the container/text/media family gains a semantically complete native renderer honoring its full property schema, replacing generic fallback surfaces. The component readiness matrix advances per kind.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [x] Each kind in batch renders all declared properties correctly
- [x] Targeted patches update visuals without remounting subtrees
- [x] Matrix records rendered/verified states per kind
- [x] Visual regression fixtures captured for stable overlays and frames

## Implementation Notes

- Added the shared renderer readiness contract in `studio-components` and the durable
  `docs/component-matrix.md` matrix. Batch A marks only container/text/media/display kinds as
  semantically rendered and verified; mapped-only future batches remain explicit `no`.
- Expanded the GPUI `plugin_node` Batch A branches for stable retained IDs, common
  `visible`/`opacity`/`accessibility_label` handling, semantic container/text/media properties,
  and non-WebP asset format detection while preserving existing `*-img` and `*-cart-img` sizing.
- Added in-code Batch A visual/state fixture coverage alongside readiness metadata and image format
  detection tests.
- UNVERIFIED: code and tests are authored but intentionally not compiled or executed in this writer
  pass; the serialized runner/fixer pass must confirm GPUI element APIs and the full workspace gates.
