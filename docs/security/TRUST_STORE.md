# Publisher trust store

Production native bundle launch requires an operator-provisioned trust snapshot. The path is
provided through `STUDIO_TRUST_STORE`; an unset variable, unreadable file, malformed document,
inactive snapshot, or snapshot with no active keys stops launch before guest instantiation. The
path and document are host configuration, not package input. Development launches selected with
`--dev` do not read this production snapshot.

## Snapshot format

Snapshots are JSON documents with schema version `1`:

```json
{
  "schemaVersion": 1,
  "snapshotId": "production-2026-08-27-a",
  "version": 42,
  "validFrom": 1787788800,
  "expiresAt": 1790467200,
  "keys": [
    {
      "publisherId": "com.example",
      "keyId": "publisher-2026-a",
      "publicKey": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "validFrom": 1787788800,
      "expiresAt": 1790467200,
      "enabled": true
    }
  ],
  "revocations": []
}
```

`publicKey` is the 32-byte Ed25519 public key encoded as 64 hexadecimal characters. Snapshot and
key windows are Unix timestamps; `expiresAt` is exclusive. Unknown fields, duplicate identities,
invalid timestamps, malformed keys, and invalid identities are rejected. `snapshotId` and
`version` are retained as safe release evidence; private signing material is never accepted or
stored by the runtime.

## Rotation and revocation

Publish a new, higher `version` before the old snapshot expires. Keep the old and new keys active
for the overlap window, and set the new key's `validFrom` to the handoff boundary. After all
bundles signed by the old key have migrated, add its `(publisherId, keyId)` to `revocations` and
remove it from future snapshots. Revocation is evaluated at snapshot load and a revoked key can
never authorize a bundle, even if its timestamp window remains open.

The snapshot is selected once at native startup and passed into host package preparation. Updates
to the file do not change an already running process; restart at the release/update boundary to
load the next snapshot. Keep the snapshot identity and version in release evidence, but never log
the JSON contents or public-key material.
