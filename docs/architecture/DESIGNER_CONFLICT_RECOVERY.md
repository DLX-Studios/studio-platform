# Designer conflict and recovery centers

Ticket 58 adds a host-independent resilience seam for Studio Designer.

## Conflict records

`studio-design` stores a `ConflictRecord` containing the complete local and
remote `CommandBatch` values, actor metadata, base revisions, and a
deterministic conflict identity. `ConflictCenter` loads these records before a
project is opened. `KeepLocal`, `KeepRemote`, and `KeepBoth` produce explicit
plans while retaining both original intents in the durable record for audit or
later manual merging.

## Recovery records

`RecoveryRecord` stores a `RecoveryBundle`: a logical
`StudioDesignSnapshot` plus ordered `JournalEntry` command batches. The
`RecoveryBundle::restore` path commits the snapshot through the existing
`DesignerPersistence` seam, then replays each journal operation through the
normal `DesignerSession` validation and commit path. A failed replay is
reported without returning a partially rebuilt session.

Recovery status distinguishes `Recoverable`, `InterruptedUpgrade`,
`Migrating`, `RestoreFailed`, `Quarantined`, and `Restored`. Quarantining or a
failed restore never removes the source snapshot or journal.

## Access and storage seams

`studio-app::resilience::ResilienceRoute` supplies pre-editor routes from the
authenticated dashboard (`/dashboard/conflicts` and `/dashboard/recovery`) or
project settings (`/projects/{project_id}/settings/{conflicts|recovery}`).
`LocalStoreDesignerPersistence` persists center records through typed
`LocalStore` batches; no SurrealDB handle or query language crosses the seam.

Cloud synchronization remains deliberately out of this ticket. Ticket 57 must
provide a production sync implementation behind `ConflictPersistence`; the
open issue 09 grilling decision and its production conflict protocol are still
an explicit release gate. Until that gate is resolved, these centers are local,
deterministic recovery and conflict records only.
