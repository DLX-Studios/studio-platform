# 48 [D]: Studio Library assets

**What to build:** Admit images, video, audio, fonts, documents, and icons with provenance, content hashes, stable identities, and deduplication. Originals preserved; deterministic runtime variants generated; unsafe SVG and unsupported codecs diagnosed at admission. Content-addressed asset store beside the LocalStore; Library panel browses, inserts, and binds assets.

*Note: refines open grilling issue 06.*

**Blocked by:** 14, 37

**Status:** ready-for-agent

- [ ] Importing an identical file twice yields one asset identity
- [ ] Original remains retrievable; runtime variants generate deterministically
- [ ] Unsupported codec and unsafe SVG produce diagnostics naming the reason
- [ ] Deletion is reference-aware with usage listing
- [ ] Panel is fully keyboard operable
