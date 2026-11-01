# 48 [D]: Studio Library assets

**What to build:** Admit images, video, audio, fonts, documents, and icons with provenance, content hashes, stable identities, and deduplication. Originals preserved; deterministic runtime variants generated; unsafe SVG and unsupported codecs diagnosed at admission. Content-addressed asset store beside the LocalStore; Library panel browses, inserts, and binds assets.

*Note: refines open grilling issue 06.*

**Blocked by:** 14, 37

**Status:** partial — asset admission/catalog/adapter foundations exist; native import/bind and decoder/variant work remains open

- [ ] Importing an identical file twice yields one asset identity
- [ ] Original remains retrievable; runtime variants generate deterministically
- [ ] Unsupported codec and unsafe SVG produce diagnostics naming the reason
- [ ] Deletion is reference-aware with usage listing
- [ ] Panel is fully keyboard operable

## Completion audit — 2026-11-01

Current source/evidence assessment: F05 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).


## Project authority follow-up — 2026-11-01

`LocalStoreDesignerPersistence::project_library` now supplies a project-scoped Library using
its authenticated ownership proof. Catalog and content-addressed blob locations are separate
per project; retained handles recheck active authority for reads and writes. An expired handle
does not revive on a later sign-in. Independently opened adapters share the catalog mutation
lock so concurrent imports do not overwrite each other. The trusted host-global constructor
remains for existing internal callers and is not the native project API. Existing global
records are not silently assigned to a project.

Validation: all three identity-isolation tests and four host Library tests pass, including
re-login with old handles still denied, independent provenance for identical bytes in different
projects, and concurrent catalog writes through independently opened adapters.

Native import/browse/bind/delete and immutable preview-byte snapshots are still unconnected.
The variant API now accepts only identity output and rejects resizing, quality changes and
format conversion with `UnsupportedVariant`, including reads of legacy transform-labeled
records. Five host Library tests pass. See the [capability matrix](../../../docs/development/LIBRARY_CAPABILITIES.md);
full decoding/transcoding remains unimplemented.
