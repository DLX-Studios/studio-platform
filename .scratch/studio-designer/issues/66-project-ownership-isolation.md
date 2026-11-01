# 66 [R/D]: Enforce native project ownership and session isolation

**Status:** implemented — host/native lifecycle path verified by deterministic tests; graphical acceptance remains in ticket 69.
**Blocked by:** none. **Source:** Constitution I; tickets 54/55; audit F01; backlog C001.

The shell resets project numbering on construction and each identity may create `local-project-1`.
Dashboard catalogs are identity-specific, but `open_or_create_focus` shares one LocalStore and
`LocalStoreDesignerPersistence` keys durable source/workspace/recovery records by project ID.
Opening a project checks authentication without proving ownership; the retained Focus entity
also survives sign-out. An authenticated second identity must never inherit another identity's
source or editor entity. A unique ID alone does not replace an ownership check.

## First failing regression

Add `crates/studio-designer/tests/project_identity_isolation.rs`. Exercise two real local identity
sessions against one temporary durable store. Create a project under each identity using the
production creation/open path; force the same legacy ID and attempt a foreign-ID open. Save a
recognizable value as Alice, sign out, sign in as Bob, and prove Bob cannot see or edit it. Also
reopen the process and create a second project. The test must fail for the current collision or
missing ownership check before the fix; it must not merely call an isolated dashboard mock.

## Acceptance

- [x] Project IDs are restart-safe and noncolliding across identities; creation failures are visible.
- [x] Host persistence/open verifies authenticated ownership and rejects foreign-ID reads/writes.
- [x] Sign-out/identity switch disposes retained Focus/workspace and cancels in-flight opens/edits.
- [x] Same-owner edit → close → cold reopen preserves durable source and workspace state.
- [x] Legacy ambiguous keys fail closed with recovery guidance; no automatic ownership assignment.

Expected scope: `studio-designer/src/bootstrap.rs`, `project_dashboard.rs`,
`studio-host/src/designer_persistence.rs`, and focused native/bootstrap/persistence tests. Add or
amend a closed persistence schema/migration contract before changing persisted ownership records.
Use the scoped commands in [completion tasks](../completion-tasks.md); do not claim that the
source trace itself is an executed exploit or a passing isolation test.

## Implementation evidence — 2026-11-01

The initial regression failed on the old production bootstrap: Bob's authenticated state
accepted an unowned `local-project-1` route. Native creation now allocates random IDs and
persists immutable authenticated ownership; every source/workspace/conflict/recovery access
checks the original active session and exact project. Project settings use the same check.
Dashboard membership alone cannot authorize opening or writing a project. Session changes
clear retained editor/panel state and obsolete async results are rejected by session and
request generation. Signing into another identity also invalidates the previous host session.

The [ownership contract](../../../docs/development/DESIGNER_PROJECT_OWNERSHIP.md) defines the
closed ownership record and deliberately does not migrate ambiguous legacy records.
`project_identity_isolation.rs` exercises real bootstrap/authentication, edits, forged catalog
entries, expired adapters and a child-process exit followed by cold reopen. The bootstrap unit
test also seeds a real legacy source record, verifies it remains untouched, and checks native
shell teardown and stale-result admission. This is host/lifecycle evidence, not a claim that a
physical GUI sign-out journey was executed.

Verification: `cargo test --locked -p studio-designer` (51 passed) and
`cargo test --locked -p studio-host --test designer_persistence` (3 passed).
