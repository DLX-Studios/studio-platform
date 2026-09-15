# Studio Bundle Contract v1

## Archive Layout

A production plugin is a byte-deterministic ZIP file with the `.studio` suffix:

```text
manifest.json
module.wasm
assets/...
signature.ed25519
```

Every entry uses ZIP `STORE` with no compression, lexicographic UTF-8 path order, the fixed DOS
timestamp `1980-01-01T00:00:00`, regular-file mode `0644`, no extra fields, and no archive or entry
comments. Paths are normalized UTF-8 relative paths. Absolute paths, traversal, backslashes, empty
segments, symlinks, hard links, devices, duplicate/case-colliding paths, undeclared entries, and
unsupported compression are rejected. Archive input is at most 16 MiB, the module at most 8 MiB,
and declared assets at most 1 MiB uncompressed in total. Streaming per-entry and aggregate limits
apply before allocation.

## Closed Manifest

```json
{
  "schemaVersion": 1,
  "id": "com.example.pos",
  "name": "Example POS",
  "version": "0.1.0",
  "publisher": {"id":"example","keyId":"dev-example-1"},
  "entry": "module.wasm",
  "sdkVersion": "^0.1.0",
  "protocolVersion": 1,
  "capabilities": ["payment.simulate", "printer.simulate"],
  "limits": {"memoryMiB":16,"eventFuel":10000000},
  "assets": []
}
```

All fields are required unless the generated schema explicitly states otherwise. Unknown fields,
duplicate capabilities/assets, invalid semantic versions, unsupported protocol/schema versions,
unknown capabilities, values above host ceilings, and inconsistent entry/asset paths fail.

## Canonical Signed Document

The packager computes SHA-256 for the exact module bytes and every declared asset, orders assets
by normalized UTF-8 path, and signs the UTF-8 bytes of an RFC 8785 JSON Canonicalization Scheme
document containing:

1. Domain separator `studio.bundle.signature.v1`.
2. The complete manifest value canonicalized by RFC 8785.
3. Module path, length, and SHA-256 digest.
4. Each asset path, length, and SHA-256 digest in sorted order.

`signature.ed25519` contains exactly the raw 64-byte Ed25519 signature. The key ID comes only from
the signed manifest. The host recomputes every value and verifies against its provisioned, enabled
trust-store key. The computed bundle digest becomes part of the plugin principal.

## Validation Order

1. Enforce archive byte and entry-count limits while reading.
2. Reject unsafe, duplicate, undeclared, or unsupported entries and decompression ratios.
3. Enforce module and aggregate asset limits.
4. Parse the manifest with a closed schema.
5. Canonicalize and compute all digests.
6. Verify publisher/key relationship and Ed25519 signature.
7. Validate plugin identity, versions, capabilities, and requested limits.
8. Validate WebAssembly features, imports, exports, and memory/table declarations.
9. Compile and instantiate only after all prior steps succeed.

Failure is terminal for that load and guest code is not executed.

## Developer Mode

Developer mode requires an explicit `--dev <local-bundle-path>` selection. It may accept an
unsigned bundle only for that launch and must display a persistent host-owned untrusted indicator.
It does not relax archive, manifest, identity, compatibility, capability, module, ABI, memory,
fuel, epoch, message, UI, action, or secret controls. Payment and printing remain simulators.

## Production Installation Boundary

Milestone one accepts `--bundle <absolute-local-path>` for one administrator-provisioned `.studio`
file. The path must resolve to a regular file and is selection, not authority: the host never
infers trust from its location. Marketplace discovery, remote download, publisher enrollment, key
recovery, and trust-store administration UI are out of scope.
