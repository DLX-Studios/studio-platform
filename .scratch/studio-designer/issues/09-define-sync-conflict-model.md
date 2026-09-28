# Define offline and cloud synchronization

Type: grilling
Status: resolved
Blocked by: 01, 03, 06

## Question

What operation-log, snapshot, asset-transfer, identity, authentication, retry, conflict-detection, resolution, deletion, recovery, and multi-device rules provide offline-first work with optional same-user cloud synchronization?

## Answer

Offline-first with optional same-user cloud synchronization only in v1. Local materialized design is the authority. Simultaneous multi-user editing, presence, live cursors, invitations, and organization collaboration stay out of scope per the map.

Each accepted local operation atomically updates materialized state and appends an idempotent outbox record. The cloud validates identity, schema, project ownership, base revision, and command invariants, assigns a per-project server revision, and returns an idempotent receipt. Clients pull by cursor with retry. Same-property, structural, deletion, and schema conflicts become explicit recoverable conflicts, never silent overwrites.

Assets transfer content-addressed by hash beside the operation log. Periodic logical snapshots bound replay time. Large media lives in the content-addressed asset store. Recovery restores the last durable transaction plus outbox replay.

See [CONTEXT.md](../../../CONTEXT.md) for the settled glossary.
