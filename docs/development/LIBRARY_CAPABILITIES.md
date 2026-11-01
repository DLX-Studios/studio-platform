# Library capability matrix

The host Library preserves original bytes, hashes them, and stores metadata/provenance. Its
admission checks currently inspect format signatures and selected text markers. Admission is
not proof that a complete media decoder can read the file. This distinction applies on every
platform; the Library does not load FFmpeg, GStreamer or a native media codec service.

| Family | Recognized formats | Current host check | Full decoding / transformation |
| --- | --- | --- | --- |
| Raster images | PNG, JPEG, WebP, GIF, AVIF | Signature/container markers | Not implemented by Library |
| Vector images/icons | SVG | UTF-8, SVG marker and prohibited-content markers | No complete XML sanitizer or rasterizer |
| Video | MP4, MOV, WebM | Container markers; synthetic `codec=` marker policy | No real stream/codec probing or transcoding |
| Audio | MP3, WAV, OGG, FLAC, M4A, AAC | Signature/container markers | No decoding or transcoding |
| Fonts | WOFF2, WOFF, TTF, OTF | Signature markers | No font validation or subsetting |
| Documents | PDF, plain text, Markdown | PDF signature or UTF-8 | No document conversion |

This table describes source-inspected behavior, not certification of every format or decoder.
`studio-host/tests/library_assets.rs` exercises original-byte preservation/deduplication,
prohibited SVG/script and synthetic unsupported-codec rejection, reference-aware deletion,
concurrent imports, and the variant policy below. It uses small signature fixtures; those tests
do not claim successful image/video/font decoding. Native asset presentation and a corpus of
valid, malformed and oversized media remain part of C007 acceptance.

## Variant contract

An identity variant uses no width, height or quality request and either omits the output format
or specifies the original format. Its bytes remain identical to the original. Its deterministic
identity and integrity checks support caching; it is not a resized or transcoded resource.

Requests with width, height, quality or a different output format return `UnsupportedVariant`
before publishing variant metadata or bytes. Reads of legacy records carrying those requests
also fail with that diagnostic, even if an earlier version stored an unchanged original under
the requested transform. Original assets remain readable. No silent fallback is reported as a
successful transformation. A future encoder implementation needs a versioned output contract
and decoder/resource-limit tests before enabling these requests.

## Project authority

Native project callers obtain the Library through `LocalStoreDesignerPersistence::project_library`.
The adapter checks project ownership and active host session authority on reads and writes.
Each project has separate catalog/blob storage. Equal bytes retain the same content identity
without sharing another project's provenance or access. Sign-in creates new authority; it never
reactivates an expired handle. Existing trusted host-global Library constructors are internal
integration paths and must not be substituted for the project API. Legacy global records are
not automatically claimed by a project.
