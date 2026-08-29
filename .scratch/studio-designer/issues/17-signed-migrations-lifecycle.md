# 17 [R]: Signed application data migrations

**What to build:** Signed data migrations execute in a dedicated pre-launch lifecycle with a created recovery point, version checks, idempotent application, and a rollback/recovery policy. Migrations always complete before ordinary guest access begins.

**Blocked by:** 14

**Status:** done

- [x] Schema v1 to v2 rehearsal includes a crash injected mid-migration and ends in a usable prior-or-new state, never a half-migrated store
- [x] Recovery point is created before execution and restorable
- [x] Failed migration quarantines the application with a safe diagnostic instead of launching
- [x] Engine upgrades remain a separately rehearsed path from schema migrations

## Closure evidence (commit on closure/01-ticket-17-signed-migrations)

### 1) v1→v2 crash rehearsal — real RocksDB, kill/restart, never half-migrated
- Before this closure the only rehearsal was `crates/studio-host/src/migrations.rs:830` `crash_mid_migration_recovers_the_prior_state_before_retry` which used `MemoryStore` + `tokio::spawn` panic — not a real engine crash.
- Fix: added real-engine rehearsal following ticket-14 pattern `crates/studio-host/src/bin/localstore_crash_worker.rs` + `crates/studio-host/tests/crash_recovery.rs`:
  - Worker `crates/studio-host/src/bin/migration_crash_worker.rs:1` opens `EmbeddedLocalStore` with `Durability::Every` at schema v1 `{"stable": true}`, admits a signed `VerifiedMigrationBundle` (v1→v2), then calls `MigrationRunner::run` whose action writes `.studio-migration-test-paused` after the host has persisted `RecoveryPointCreated` → `Applying` and then parks forever. The parent `crates/studio-host/tests/migration_crash_recovery.rs:89` waits for the marker (20s deadline), `kill`s the worker, `wait`s, reopens the same directory with `EmbeddedLocalStore::open`.
  - Assertions on reopen (`crates/studio-host/tests/migration_crash_recovery.rs:117`): `metadata().engine_format_version()==1` unchanged, `MigrationState` has no `{"half":true}`, either `Committed` v2 or recoverable `RecoveryPointCreated|Applying|Validating` at v1 with `recovery_point().is_some()` and `data=={"stable":true}`.
  - Retry `runner.run` with a real action completes atomically: `initial_version==1`, `final_version==2`, `applied==["v1-to-v2"]`, `data=={"stable":true,"schema":2}`, `lifecycle==Committed` (`crates/studio-host/tests/migration_crash_recovery.rs:145`). Second `run` is idempotent no-op (`crates/studio-host/tests/migration_crash_recovery.rs:168`), and `engine_format_version` still `1`.
- Reused commit cache `CARGO_TARGET_DIR=/home/sir/.cache/studio-platform-target-275020a-debug0` per `AGENTS.md`.

### 2) Recovery point created before execution and restorable
- Code `crates/studio-host/src/migrations.rs:373-383` creates `RecoveryPoint { id: recovery_id(package, schema_version), schema_version, data.clone(), completed.clone() }` and persists `RecoveryPointCreated` *before* iterating declarations; each `Applying`/`Validating` is persisted before/after the candidate.
- `crates/studio-host/src/migrations.rs:286-306` `restore_recovery_point` is an explicit host operation that writes `schema_version/data/completed` from the point, clears `recovery_point`, sets `Idle`.
- Idempotency test `crates/studio-host/src/migrations.rs:710` `migration_run_is_idempotent_and_retains_a_recovery_point` asserts `recovery_point_id.is_some()` and replay is no-op.
- Quarantine test `crates/studio-host/src/migrations.rs:746` `failed_migration_quarantines_and_explicit_restore_recovers_prior_data` restores and checks `Idle` + `data=={"stable":true}`.
- RocksDB reopen evidence `crates/studio-host/tests/migration_crash_recovery.rs:122` interrupted retains recovery point.

### 3) Failed migration quarantines with safe diagnostic instead of launching
- Runner `crates/studio-host/src/migrations.rs:399-445` on `ActionFailed`/`ValidationFailed` restores `schema_version/data/completed` from `recovery_point`, persists `Quarantined { migration_id }`, returns `MigrationError::ActionFailed|ValidationFailed`; next `run` without restore returns `MigrationError::Quarantined` (`crates/studio-host/src/migrations.rs:341-342`).
- Safe diagnostics: `MigrationErrorCode::AdmissionInvalid|StateCorrupt|Quarantined|...` with `Display` strings containing no storage/package/application-data details (`crates/studio-host/src/migrations.rs:131-188`). Verified in `crates/studio-host/tests/migration_crash_recovery.rs:252` `err.to_string()=="application migration is quarantined; restore the recovery point before retrying"` and does not contain payload.
- RocksDB persistence `crates/studio-host/tests/migration_crash_recovery.rs:207` `signed_migration_quarantine_survives_reopen_and_requires_explicit_restore` proves quarantine survives `close`+`open` and still blocks `run` until `restore_recovery_point`.
- Validator half-migration test `crates/studio-host/src/migrations.rs:781` + RocksDB `crates/studio-host/tests/migration_crash_recovery.rs:303` both assert `data=={"stable":true}` and `schema_version==1` after `ValidationFailed`.
- Host gating `crates/studio-app/src/host.rs:238-355`: `StudioHost::prepare` returns `LaunchError::MigrationRequired` when `manifest.migrations` non-empty and `migrations_complete==false`; `prepare_with_migrations` (`crates/studio-app/src/host.rs:247-295`) admits `VerifiedMigrationBundle::admit` (unconditional signature trust), runs `MigrationRunner::run`, maps any `MigrationError` to `LaunchError::MigrationInvalid` *before* `prepare_internal` instantiates WASM or mounts UI — so a half-migrated or quarantined store never launches a guest.
- Existing `crates/studio-package/tests/migration_admission.rs:89` ensures `VerifiedMigrationBundle::admit` requires a valid publisher signature.

### 4) Engine upgrades separately rehearsed from schema migrations
- `crates/studio-host/src/migrations.rs:222-226` documents `MigrationRunner` never changes `StoreMetadata::engine_format_version`; engine/on-disk upgrades remain `crate::EmbeddedLocalStore` recovery path.
- Separate engine rehearsal `crates/studio-host/tests/crash_recovery.rs:23` `forced_termination_recovers_the_last_durable_transaction_without_partial_batch` uses the same `Durability::Every` + worker kill pattern but for `studio_store_batch` atomicity, not migration logic.
- Migration rehearsal `crates/studio-host/tests/migration_crash_recovery.rs:117,179` asserts `metadata().engine_format_version()==1` before and after migration, and `metadata().schema_version==1` (store schema) unchanged, while `MigrationState.schema_version` moves 1→2.

## Verification (AGENTS.md — focused, low-footprint)
- `CARGO_TARGET_DIR=/home/sir/.cache/studio-platform-target-275020a-debug0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo fmt --all -- --check` → ok
- `cargo test --locked -p studio-host --lib migrations` → 5 passed (`crates/studio-host/src/migrations.rs`)
- `cargo test --locked -p studio-host --test migration_crash_recovery -- --nocapture` → 3 passed (real RocksDB crash, quarantine reopen, validator RocksDB)
- `cargo test --locked -p studio-package` → ok (tail shows 4+5+4 tests passed)
- No `cargo test --locked --workspace`, no `cargo clippy --locked --workspace --all-targets`, no release build (per AGENTS.md reuse cached host artifacts).

## Remaining external gaps (honest)
- Designer UI recovery/quarantine surfaces for interrupted upgrades and quarantined projects (dashboard, project settings, conflict center) are out of scope here — tracked in `.scratch/studio-designer/issues/58-conflict-recovery-centers.md` and spec item 171.
- End-to-end `StudioHost::prepare_with_migrations` with a real `.studio` bundle file on disk and a host-owned `EmbeddedLocalStore` directory was verified at the `MigrationRunner` + `VerifiedMigrationBundle` boundary; a file-level integration test that writes the bundle to a temp file and calls `host.prepare_with_migrations` → `host.prepare` was not added (host gating is covered by code-level `crates/studio-app/src/host.rs:354` and unit tests).
- `STUDIO-BENCH-1` throughput qualification and release artifact scans remain external gates (see `AGENTS.md` integration checkpoint).
- No change to `engine_format_version` migration; future engine major bumps will need their own rehearsal separate from this application-schema `MigrationState`.
