# Define Studio Library ownership and bindings

Type: grilling
Status: resolved
Blocked by: 03

## Question

What asset, content-collection, content-binding, provenance, deduplication, lifecycle, packaging, and runtime snapshot rules let users, agents, extensions, and compiled applications share Studio Library content safely and predictably?

## Answer

Studio Library is strictly project-owned in v1. Each project owns its assets, content collections, bindings, and forms. Users, agents, extensions, MCP clients, and compiled applications share content only through that project's snapshot. No cross-project or global libraries in v1.

Asset identity is content-addressed with a stable opaque ID (`asset-sha256-{hash}`). Importing identical bytes twice yields one identity and merges provenance. Originals are preserved byte-for-byte; deterministic runtime variants are derived by variant key. Deduplication is by exact SHA-256 source bytes; blobs are content-addressed under `blobs/sha256/`.

Every asset and collection record carries provenance (source, actor, kind, detail). Provenance is opaque metadata, never used to locate blobs. Admission validates formats by extension plus magic bytes, gates video/audio codecs to an approved list, and rejects unsafe SVG. Unsupported format, unsupported codec, and unsafe SVG fail at admission with a named diagnostic; unsafe content is never packaged.

Content bindings are typed references (`PropertyValue::Asset`, collection binding with expected kind plus optional fallback). Type mismatch or missing target is a build error (`CONTENT_BUILD_BLOCKED`) unless a valid type-correct fallback is declared. Failures notify through Designer diagnostics with node/binding IDs, the projection report, and an immediate editor toast/banner.

Lifecycle is reference-aware. Assets track usages (`reference_id`, owner, field). Deletion defaults to `RequireUnbound` with a usage listing; breaking deletes are explicit and report broken references. Collection schema changes surface affected bindings as diagnostics.

Packaging ships an immutable library snapshot inside the signed Runtime package. The projection maps assets to `assets/<path>` strings; bytes never cross the design seam. Manifest asset lists must exactly match packaged keys; archives are deterministic with fixed sizes and permissions.

See [CONTEXT.md](../../../CONTEXT.md) for the settled glossary.
