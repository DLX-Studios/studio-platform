# Environment isolation

Three deployment environments — `development`, `staging`, `production` — coexist on one machine with independent data and credential namespaces. Packages declare secret names and purposes only; the active environment supplies the values.

## Partitioning

Both the embedded data layer and the protected credential layer derive per-application, per-environment partitions with domain-separated, length-delimited SHA-256.

| Layer | Domain | Derivation input |
| --- | --- | --- |
| Data (`EnvironmentDataStore`) | `studio.environment.data-partition.v1` | `application` + `environment` |
| Credential (`ProtectedSecretStore` / `ApplicationPartition`) | `studio.protected-secret.partition.v1` + `studio.protected-secret.credential.v1` | `publisher_id` + `plugin_id` + `environment` |

Because the domains differ, a data key and a credential locator never alias even for the same logical name, application, and environment. Each `EnvironmentDataScope` mints keys bound to its partition digest (`encode_hex(partition)`), and `admit` refuses any key minted elsewhere with `environment.cross_environment_denied`. Credential locators are second-preimage hashes of the credential partition digest plus declared name/purpose and are opaque to all callers.

Three environments therefore resolve to three independent data keyspaces and three independent credential keyspaces. Cross-environment, cross-application, and cross-publisher access are all refused with stable safe codes.

## Active-environment selection

`resolve_active_environment` reads the host-protected configuration key `environment.active` from `ProtectedConfiguration`. There is no default:

| Condition | Error code |
| --- | --- |
| No entry for `environment.active` | `environment.config_missing` |
| Unrecognized value (case-insensitive match against `development`/`staging`/`production` only) | `environment.config_invalid` |
| Multiple distinct values for the same key | `environment.config_ambiguous` |

Values are never echoed in diagnostics (`safe_description` is the only diagnostic string). `ProtectedConfiguration` retains duplicate keys so ambiguity can be diagnosed instead of silently resolved.

`EnvironmentDataStore::active_scope` and `scope_for_principal` enforce the same development-principal rule as `ProtectedSecretStore::for_application`: `TrustMode::Development` principals may only address `development`.

## Secret resolution

Secret status follows the active environment partition. `ProtectedSecretStore::for_application` derives the same environment-sensitive partition for credentials; a staging secret is `Configured` only in `staging` and `Missing` in `development` and `production`. The test `secret_resolution_follows_the_active_environment_not_the_package` configures a restricted key in staging and asserts the other two environments remain `Missing`.

## Secret-free promotion

Promotion is structurally incapable of moving credential material:

* **Type-system half:** `PromotionPlan::build` accepts only `SecretFreeMetadata`, a sealed trait implemented solely for `ProtectedSecretStatus` (fields: validated name, purpose, lifecycle state, revision — no byte buffer). No guest-reachable type satisfies the bound.
* **Structural half:** `apply_promotion` receives no `CredentialBackend` handle, no `SecretInput`, and no `CredentialBytes`. It copies only `BTreeMap<String, Vec<u8>>` data records and returns a `PromotionReceipt` containing the direction, a count, and the names that still require fresh configuration in the target.
* **Runtime proof:** `promotion_copies_zero_secret_material` snapshots the entire credential backend (`HashMap<CredentialLocator, Vec<u8>>`) before and after promotion and asserts byte equality; it also asserts the rendered `Debug` plan never contains the staging credential bytes and that `production` remains `Missing`.

Separate namespace proof: `localstore_and_secret_namespaces_are_separate_and_independent` configures a staging secret and mints a data key for the same logical name, shows admission and status are independent, then promotes data records and re-asserts the credential snapshot is unchanged and production secrets remain `Missing`.

## Promotion refusal for invalid identities

`apply_promotion` refuses invalid source or target identities with stable codes:

| Invalid identity | Code |
| --- | --- |
| Source or target environment mismatches `PromotionDirection` (e.g., `StagingToProduction` with `Development`→`Staging` scopes, swapped envs, same-env) | `environment.cross_environment_denied` |
| Source and target belong to different `application` strings | `environment.cross_environment_denied` |
| Any record logical name fails `validate_logical_key` (empty, `../escape`, `bad/key`, >256 bytes, non-alphanum except `._-`) | `environment.request_invalid` |
| Empty or oversized `application` at `EnvironmentDataStore::new` | `environment.request_invalid` |

`promotion_refuses_invalid_source_target_identities` exercises all four families, including cross-application, same-environment, swapped, `DevelopmentToStaging`→`Production` mismatch, and malformed logical names. Diagnostics never echo the invalid value.

## Production-real backend evidence and external gate

The most production-real backend is `OsCredentialBackend` (keyring 4.1: macOS Keychain Services, Windows Credential Manager, freedesktop Secret Service). Deterministic tests use `MemoryBackend` for hermetic proofs; one gated integration test additionally exercises the OS facility when available:

`os_credential_backend_evidence_with_explicit_external_gate`:

* Checks `OsCredentialBackend::shipped_facility()` — on mobile targets this is `None` and the test records an external gap.
* Checks `OsCredentialBackend::is_available()` — on headless Linux without D-Bus/Secret Service this is false and the test records an external gap.
* Otherwise creates a unique publisher/application/declaration per invocation (hex suffix from time/pid/thread) to avoid vault pollution, purges leftovers, configures a staging secret through the real backend, asserts staging `Configured` and production `Missing`, builds a value-free `PromotionPlan`, applies a data promotion, asserts the OS production partition is still `Missing`, and purges both partitions.
* If any individual OS operation returns `BackendUnavailable` (locked vault, D-Bus unavailable), the test records the external gap and returns rather than faking a pass.

In the current CI container the Secret Service is available (`freedesktop Secret Service`), and the gated test passes end-to-end, reporting `production-real evidence: OsCredentialBackend (freedesktop Secret Service) proved isolated staging secret and secret-free promotion`. When the service is unavailable the deterministic `MemoryBackend` suite still proves all isolation properties; the external gap is explicitly logged instead of inventing OS-service claims.

## Diagnostics

All environment-layer failures use `EnvironmentErrorCode` with stable wire-safe codes (`environment.request_invalid`, `environment.config_missing`, `environment.config_invalid`, `environment.config_ambiguous`, `environment.cross_environment_denied`) and redacted `Display` strings. No provider value, configuration value, or secret byte appears in logs, histories, backups, or sync payloads.

## Tests

* `crates/studio-security/tests/environment_isolation.rs` — 9 tests covering three-environment coexistence, active selection without defaults, secret resolution per environment, secret-free promotion with byte-equality proof, cross-environment denial matrix, malformed-input stable codes, separate LocalStore/secret namespaces, invalid-identity promotion refusal, and OS-facility evidence with external gate.
* `crates/studio-security/tests/protected_store.rs` and `principal_policy.rs` — partition and admission baselines reused by the environment isolation proof.

Verification: `CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/sir/.cache/studio-platform-target cargo test --locked -p studio-security` (plus `cargo fmt --all -- --check` and `cargo clippy --locked --workspace --all-targets -- -D warnings`).
