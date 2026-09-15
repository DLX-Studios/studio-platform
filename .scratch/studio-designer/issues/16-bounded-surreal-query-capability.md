# 16 [R]: Bounded Surreal query capability

**What to build:** Applications that declare the capability in their signed manifest may run SurrealQL through the host with host-side parameter binding only, permanently locked to their own namespace. All other applications are unaffected.

**Blocked by:** 15

**Status:** implemented

- [x] Parameters bind host-side; string interpolation into query text is impossible through the interface — `crates/studio-host/tests/bounded_surreal_query.rs:parameter_binding_is_host_side_and_injection_safe` proves host `BTreeMap<String, Value>` binding only; missing `$name` is `QueryInvalid`; malicious payload `"; DROP ...` is safely bound as a value and returns `[]` without dropping the table.
- [x] Namespace or database switching inside queries is rejected — `every_rejected_keyword_produces_forbidden` covers `USE`/`INFO` etc. as `Forbidden`; `namespace_table_rewriting_confines_to_allowlist` and `namespace_isolation_between_principals` prove opaque `appdata_v{version}_{hex}_{collection}` rewriting and cross-namespace isolation.
- [x] System data, filesystem, scripting, and network functions reachable from queries are denied — `every_rejected_keyword_produces_forbidden` covers `system`, `information_schema`, `meta`, `auth`, `session`, `file`, `http`, `https`, `script`, `javascript`, `js`, `function`, `fn`, `os`, `process`, `filesystem`, `sleep`, plus `system_markers_double_colon_forbidden_and_safe` for `::`.
- [x] Query size, result size, and duration limits are enforced and configurable per declaration — `query_size_limit_enforced` (`QueryTooLarge`), `result_size_limit_enforced` (`ResultTooLarge`), `time_limit_enforced_via_host_timeout` (`TimedOut` via mock `SurrealQueryStore` respecting `max_duration`), `declaration_limits_validation` (`DeclarationInvalid` for zero/over-ceiling; `SurrealQueryLimits::new` host ceilings 64 KiB / 4 MiB / 10 s; per-declaration `SurrealQueryDeclaration::new(limits)`).
- [x] Denied attempts produce safe, source-linked diagnostics — `denied_attempts_produce_safe_source_linked_diagnostics` and `system_markers_double_colon_forbidden_and_safe` prove `QuerySource { line, column }` is echoed, `SurrealQueryError::to_string()` is fixed context-free (`surreal query operation denied`, `surreal query is invalid`, etc.), and no payload / namespace digest / query text leaks via `Display` or `Debug`. Multi-statement `;` is `QueryInvalid`, undeclared tables are `CollectionUndeclared`, capability absence is `CapabilityDenied` (`fail_closed_on_undeclared_tables_and_capability_denied`).

**Evidence bundle**

- Implementation: `crates/studio-host/src/application_data.rs:444-847` (`SurrealQueryLimits/Declaration/Request/Response`, `scope_query` with `lex_query`, allowlist + `FORBIDDEN` list + `::` + `;` checks, `scoped_table_name` opaque rewriting, `validate_parameters`, host `max_query_bytes/max_result_bytes/max_duration` enforcement, `QuerySource` linking, `ApplicationDataQueryGuestApi` guest trait) and `crates/studio-host/src/local_store.rs:498-786` (`SurrealQueryStore::execute_surreal_query` host-side binding, `tokio::time::timeout`, safe `LocalStoreDiagnosticCode::QueryTimedOut` mapping). Manifest admission: `crates/studio-package/src/manifest.rs:141` `Capability::DataSurrealQuery` and `crates/studio-security/src/capability.rs:15` `CapabilityId::DataSurrealQuery`.
- Integration suite: `crates/studio-host/tests/bounded_surreal_query.rs` — 17 tests, all passing under the ticket's exact verification command (see below). Covers: declared query executes and returns rows (seed via `CREATE` then `SELECT`); parameter binding only; namespace/table rewriting; isolation; every forbidden/system-marker class; multi-statement; query/result/time limits; fail-closed on undeclared; `CapabilityDenied`; safe diagnostics; typed-collection interop boundary; complex JOIN/WHERE/ORDER/LIMIT rewriting.
- Verification (per `AGENTS.md`, focused only, reuse exact-commit cache):

  ```text
  export CARGO_TARGET_DIR=/home/sir/.cache/studio-platform-target-275020a-debug0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
  cargo fmt --all -- --check
  cargo test --locked -p studio-host --test bounded_surreal_query -- --test-threads=1
  # → 17 passed; 0 failed
  ```

**Current contract — typed-collection interop boundary (honest)**

Typed collection helpers (`ApplicationDataHandle::create/select/list/update_merge/update_patch/delete`) persist via the `LocalStore` batch API at `appdata.v{version}.{hex}.{collection}` (see `application_data.rs:1452`). Bounded SurrealQL persists via SurrealDB opaque tables at `appdata_v{version}_{hex}_{collection}` (see `scoped_table_name`). Both share the same `EmbeddedLocalStore` instance and the same `ApplicationDataNamespace` derivation, but they use disjoint storage mechanisms. Therefore **typed rows are not visible via SurrealQL and SurrealQL rows are not visible via typed helpers** — there is no implicit mirroring. This is the audit-flagged known limitation ("typed collection persistence is not mirrored into query tables"). It is proven by `typed_collection_persistence_is_not_mirrored_into_query_tables`: a typed `create` is absent from `SELECT * FROM catalog` and a query `CREATE` is absent from `list("catalog")`. A future follow-up may mirror typed collections into query tables, but the current contract is intentionally disjoint and documented here. No silent fallback: querying an empty (never-created) opaque table returns `ExecutionFailed` (Surreal `Table does not exist`), not an empty array; after at least one `CREATE` the table exists and `SELECT` returns rows.

**Remaining external / follow-up gaps (honest)**

- No live hardware or Stripe/GitHub staging is required for this ticket, but the broader release needs three physical stations, live `wss` center, and hardware printers — tracked as flagship release gaps, not ticket-16 defects.
- Surreal `SELECT * FROM <empty-table>` currently surfaces as `ExecutionFailed` rather than `[]` because Surreal 3.2.4 with `Capabilities::none` reports `Table does not exist`. This is host-faithful but differs from a hypothetical empty-set semantic; no ticket requires changing it now. Workaround is to `CREATE` before `SELECT` (covered in tests).
- Mirroring typed collections into query tables remains a follow-up design decision (see contract above); closing ticket 16 does not claim that invariant.
