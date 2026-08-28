# Native product bootstrap

`studio-app` now opens the host `EmbeddedLocalStore` with `Durability::Every`
before creating a GPUI window. The production identity service uses
`OsSessionCredentialStore`, reads the identity catalog, and attempts to resume
the first available remembered session. Store, catalog, credential, and schema
failures terminate startup with sanitized diagnostics; an unauthenticated
project surface is never constructed.

`NativeProductBootstrap` owns the composed services. Its dashboard and settings
accessors use `LocalStoreDashboardPersistence` and
`LocalStoreSettingsPersistence`, which serialize one validated record per
identity/project into typed host batches. No SurrealDB handle or query language
crosses into the app shell.

`NativeProductState` is the renderer-neutral route model. A clean device starts
at `/welcome`, a dismissed welcome reaches `/identity`, and a resumed session
starts at `/dashboard`. Settings, help, about, sync status, conflicts, and
recovery are dashboard-level routes; project routes require an active session.
The shared `ConnectionIndicator` is sourced from the offline-first
`SyncCoordinator`, so local and cached projects remain usable when cloud
transport is unavailable.

The GPUI `NativeProductShell` renders the route model and retains a verified
Runtime surface only after authentication. The existing foundation renderer is
therefore a child of the authenticated project route rather than the native
startup surface.
