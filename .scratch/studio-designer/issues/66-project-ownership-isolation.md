# 66 [R/D]: Enforce native project ownership and session isolation

**Status:** open — Critical, source-confirmed collision path; executable reproduction pending.
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

- [ ] Project IDs are restart-safe and noncolliding across identities; creation failures are visible.
- [ ] Host persistence/open verifies authenticated ownership and rejects foreign-ID reads/writes.
- [ ] Sign-out/identity switch disposes retained Focus/workspace and cancels in-flight opens/edits.
- [ ] Same-owner edit → close → cold reopen preserves durable source and workspace state.
- [ ] Legacy ambiguous keys fail closed with recovery guidance; no automatic ownership assignment.

Expected scope: `studio-designer/src/bootstrap.rs`, `project_dashboard.rs`,
`studio-host/src/designer_persistence.rs`, and focused native/bootstrap/persistence tests. Add or
amend a closed persistence schema/migration contract before changing persisted ownership records.
Use the scoped commands in [completion tasks](../completion-tasks.md); do not claim that the
source trace itself is an executed exploit or a passing isolation test.
