# 18 [R]: Protected secret store and protected configuration

**What to build:** One host-owned credential store partitioned per verified application identity, backed by the operating-system credential facility. Packages declare secrets by name and purpose only; publisher-configured values arrive out-of-band, user-supplied values through a host-owned entry surface. Guests observe status metadata only; injection happens exclusively inside network brokers; rotation never requires rebuilding or re-signing packages.

**Blocked by:** None (can start immediately)

**Status:** done

- [x] Cross-partition reads between applications are impossible through any interface
- [x] No guest-callable read path for secret values exists; only configured/missing/revoked status
- [x] User-supplied key entry captures the value without ever displaying it again
- [x] Rotation replaces values while package signatures stay valid
- [x] Release gate proves shipped artifacts contain no default credentials and known defaults fail authentication
- [x] Redaction scrubs key-shaped values from logs, diagnostics, and crash reports

## Implementation notes

- `crates/studio-security/src/protected.rs` — `ProtectedSecretStore` derives an app/environment
  partition (`ApplicationPartition::derive`) via domain-separated SHA-256 over
  (publisher_id, plugin_id, environment); credential locators are second-preimage hashes of
  partition digest + declared name/purpose, so no cross-application oracle exists and locators are
  opaque to any caller.
  - `CredentialBackend` trait abstracts the OS credential facility; `OsCredentialBackend` ships
    macOS Keychain Services, Windows Credential Manager, or freedesktop Secret Service per target
    (`shipped_facility()` documents this; mobile has none). Deterministic test backends run on CI.
  - Guest surface is `GuestSecretStatusApi` (sealed trait) returning `ProtectedSecretStatus`
    (missing/configured/revoked + revision only). The type system blocks value reads: secret bytes
    live in non-`Clone`, non-serializable `Zeroizing` wrappers with redacted `Debug`; only the
    trusted backend and the broker send-time callback receive borrowed bytes.
  - Broker hook: `BrokerSecretInjector::inject_at_send_time(key, &mut dyn BrokerCredentialSink)`
    — the interface ticket 19's REST brokers will call; no network broker implemented here.
- `crates/studio-security/src/redaction.rs` — `SensitiveValueFilter` scrubber: registered exact
  values, credential-labeled `key: value` / header patterns, known token prefixes (sk_live_,
  ghp_, AKIA…, JWTs), recursive JSON sanitization, plus a persistence-rejection check. Wired into
  diagnostic paths in `tests/security/redaction.rs`.
- `crates/studio-package` — manifest v1 accepts `secrets` declarations by name + purpose only;
  preflight/signing validates declarations; rotation changes stored values without touching signed
  package bytes (`tests/protected_secret_manifest.rs`, manifest_v1 coverage).
- Release gate: `crates/studio-security/tests/release_defaults_gate.rs` rejects six known default
  credentials at capture and injection time and scans compiled artifacts for them.
- Docs: `docs/security/PROTECTED_SECRET_STORE.md`.
- Gates: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets --
  -D warnings`, `cargo test --locked --workspace` all green under coordinator-granted exclusive
  build rights.
