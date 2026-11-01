# Designer project ownership contract (version 1)

Native creation allocates a cryptographically random 128-bit `project-<hex>` identifier.
It records an immutable ownership proof through `IdentityService.write_project_batch`
under the authenticated identity, using `designer-owner-<project-id>` as the scoped key.
The closed JSON record contains `schema_version: 1`, `project_id`, and `identity_id`.
Only creation of a fresh generated ID may write this record. Opening an arbitrary ID
never claims it. Existing source, workspace, recovery and conflict records retain their
version-1 encoding and project keys; every native access requires the ownership proof.

An adapter is bound to one opaque host session and one project. Every read and write
checks that the session is still active, the requested project matches, and the durable
ownership proof has the supported version and exact identity/project. Native routing
alone grants no persistence authority. Sign-out invalidates retained adapters; the shell
drops the editor and discards asynchronous results from prior sessions/routes.
Operations admitted before sign-out may finish their existing durable transaction;
subsequent operations must be rejected. Session state must never be installed into a
new identity's shell after sign-out.

Unowned legacy records are not adopted on first open, even when a dashboard contains
that ID. They remain intact and inaccessible through the native editor. Recovery must
explicitly establish ownership through a future migration/import operation; copying
an ID into a dashboard is not such proof. Failed creation may leave an unlisted owned
record, but cannot grant another identity access or reuse an existing source key.

The unrestricted persistence constructors remain trusted host/test adapters for existing
non-product callers. Authenticated native code must use the session-bound constructor.
These controls protect the application identity boundary; they do not claim to prevent
an operating-system user from modifying their own database files or trusted host code.
