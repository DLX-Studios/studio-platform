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
