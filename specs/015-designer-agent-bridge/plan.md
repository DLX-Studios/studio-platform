# Follow-up implementation plan

This plan covers T015-6 and T015-7. The authenticated Designer project remains
the owner of local workspace preferences, `studio dev` remains the owner of
the live preview, and source files remain the persistence boundary between an
agent and the preview.

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

The current `studio dev` compiler mounts one `.studio` entry per session. This
ticket reports that mounted entry and does not claim screen switching. The
pinned GPUI API also lacks a general element transform, so non-default visual
zoom is tracked with multi-screen support in T015-8 rather than represented as
an accepted control that has no effect.

## Validation

Add focused tests for workspace preference compatibility, clean and dirty
source refresh, stale-safe revert, JSON-RPC validation, and presentation range
checks. At the integration checkpoint run format, workspace tests, and Clippy
sequentially with the shared Cargo target, then the focused native hot-reload
check when a display is available.
