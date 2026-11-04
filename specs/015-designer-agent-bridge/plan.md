# Follow-up implementation plan

The authenticated Designer project owns local workspace preferences,
`studio dev` owns the live preview, and the project folder's source files are
the persistence boundary between an agent and the preview. Compressed `.studio`
archives remain build/release artifacts; dev reads the folder directly.

## T015-6: agent edits in Designer

1. Persist an optional canonical local source root on the workspace record
   keyed by the authenticated `ProjectId`. Older workspace records must still
   deserialize.
2. Add a bounded project-local source-change notification endpoint. Successful
   MCP writes and patches publish the path, before/after revisions, previous
   content when available, and the client's reported name. Notifications are
   advisory; disk revisions remain authoritative.
3. Add a source document model with base, local, and incoming versions. Clean
   buffers accept incoming disk content. Dirty buffers preserve local text and
   expose a conflict with a diff and agent attribution.
4. Add a separate file-backed panel to Designer. Save uses the buffer's base
   revision; accepting the incoming version discards local edits; reverting an
   agent write is guarded by the current disk revision.

## T015-7: controls for the mounted preview entry

1. Extend the existing bounded local dev JSON-RPC endpoint with selection and
   presentation state plus validated setter methods. Socket requests must hand
   commands to the GPUI thread through a bounded queue and a bounded reply wait.
2. Make preview authoring selection opt-in. Report stable node/component IDs
   and source path/span where the `.studio` compiler retains that information.
3. Report and control viewport dimensions and orientation. Validate all input
   ranges before resizing the live window.
4. Relay preview state and commands through Designer MCP tools and expose the
   connection, selection mode, source location, and presentation controls in
   the Designer UI.

## T015-8: folder-based source project

1. Add optional root `studio.project.json` with schema version 1 and ordered
   `screens`, each containing a stable lowercase slug `id` and a normalized,
   project-relative `.studio` `source` path. A project without this descriptor
   keeps the current single-entry `.studio` or AssemblyScript behavior.
2. Reject unknown descriptor fields, unsupported versions, empty or oversized
   screen lists, duplicate IDs, non-normalized or escaping paths, files outside
   the canonical project root, non-`.studio` entries, and source files above
   the compiler's existing source-size ceiling.
3. Compile each declared file from the folder directly and combine its
   component definition into one in-memory module in declaration order. No
   `.studio` package archive is created by dev.
4. Keep the descriptor and declared sources in content-hash invalidation and
   retain the source path per screen so selection resolves to the right file.
   The first declared screen is the initial route; subsequent screen changes
   retain that identity across reloads when it remains declared.

## T015-9: multi-screen routing

1. Preserve the active screen ID through source reloads if it remains present;
   fall back to the first declared screen if it was removed.
2. Apply screen changes over the existing bounded JSON-RPC command queue on
   the GPUI thread. Return the active screen identity and source map.
3. Show declared screen IDs and source links in Designer and expose screen
   selection to MCP clients.

## T015-10: visual zoom

The pinned GPUI API has no general element transform. Zoom therefore scales
layout inputs and text metrics before GPUI lays out the native preview. GPUI
then derives hitboxes from the scaled layout, keeping click and selection
targets aligned. Validate zoom bounds before applying changes and test both
rendered geometry and pointer activation at multiple scales.

## Validation

Add focused tests for workspace preference compatibility, source conflicts,
descriptor validation and path confinement, module composition and source
maps, route selection and reload preservation, RPC queue bounds, zoom ranges,
and hit targets under scaled layouts. At the integration checkpoint run
format, workspace tests, and Clippy sequentially with the shared Cargo target,
then the focused native hot-reload check when a display is available.

## T015-12: native project command toolbar

Expose the same CLI process service used by MCP in the native Designer project
view. A folder picker links a canonical source root to the authenticated project
and starts `studio dev <root>` in a separate native preview window. Keep the
Designer editor window open. Reuse the existing local JSON-RPC endpoint for
readiness, selection, screen, viewport, and zoom; do not introduce a second
preview implementation or a second wire protocol.

The project toolbar owns asynchronous CLI work and bounded process output. Show
Open preview, Restart, Stop, Check, Build, task cancellation, and output with
states that distinguish a spawned process from a ready preview. Preserve
external preview ownership. Dispose owned processes when changing folders or
closing the project; reject late responses from a previous folder. Existing
dirty-buffer guards apply before linking a different folder. Provide keyboard
access through standard buttons and a folder-open shortcut. Verify the real
toolbar handlers with GPUI interaction tests, the process lifecycle tests, and
a native Wayland smoke run.
