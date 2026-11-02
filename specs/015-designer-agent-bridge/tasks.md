# Tickets

## T015-1 — Add a bounded source workspace for project files

- [x] Resolve and retain one canonical project root.
- [x] List and read UTF-8 source files using project-relative paths.
- [x] Write complete files and apply byte-range patches only when the caller's
  expected SHA-256 revision matches the current file.
- [x] Reject absolute/traversal paths, symlink escapes, generated files,
  `build`, `.git`, and dependency/cache directories.
- [x] Publish writes with same-directory temporary files and atomic rename.
- [x] Cover stale revisions, invalid UTF-8/ranges, path escapes, exclusions,
  and successful round trips.

## T015-2 — Define the Designer MCP/JSON-RPC stdio contract

- [x] Implement the supported MCP initialize/tools handshake and newline JSON
  framing with strict message-size bounds.
- [x] Expose `project.list_files`, `project.read_file`, `project.write_file`,
  and `project.apply_patch` tools with JSON schemas and structured results.
- [x] Keep stdout protocol-only; send diagnostics to stderr.
- [x] Return standard JSON-RPC errors for malformed protocol requests and
  tool-level errors for file conflicts or denied paths.
- [x] Add protocol round-trip and malformed/oversized frame coverage.

## T015-3 — Add a local JSON-RPC status endpoint to `studio dev`

- [x] Add a separate project-local socket and versioned status method; leave
  `studio-reload/1` unchanged.
- [x] Track applied code revision, compile/commit state, and last diagnostic.
- [x] Read status without blocking or mutating GPUI state; bound each request.
- [x] Remove stale socket files and clean up the endpoint on shutdown.

## T015-4 — Relay preview status through Designer

- [x] Add `preview.status` to the Designer agent tools.
- [x] Resolve the Studio status endpoint from the same canonical project root.
- [x] Report a clear `not_running` state when the project has no active preview.
- [x] Verify a source edit is reflected in preview status after reload.

## T015-5 — Document setup and verify the agent-to-preview path

- [x] Document Codex/OpenCode local MCP configuration and supported tools.
- [x] Add an integration test that sends JSON-RPC tool calls, writes a project
  source file, and confirms `studio dev` observes the changed content hash.
- [x] At the integration checkpoint, run format, workspace tests, and Clippy
  sequentially, then a focused native hot-reload check where the display
  environment permits.

## Follow-up tickets

These build on the source and preview RPCs after the first milestone lands.

### T015-6 — Connect agent edits to the open Designer project view

- [x] Persist an optional canonical source root per authenticated `ProjectId`,
  with backward-compatible workspace-record decoding.
- [x] Publish bounded local change notifications after successful MCP writes
  and patches, including revision-guarded before content when available.
- [x] Add a source buffer model that refreshes clean buffers and preserves dirty
  buffers as explicit base/local/incoming conflicts.
- [x] Add Designer link/open/edit/save controls backed by project-relative file
  access and expected-revision writes.
- [x] Show diff and reported agent name; accept incoming content or revert only
  when the disk revision still matches the agent edit.
- [x] Test project isolation, clean refresh, dirty conflict, and stale revert.

### T015-7 — Bridge selection and viewport controls for the mounted preview

- [x] Extend the bounded dev RPC with opt-in selection, selected source spans,
  mounted-entry identity, viewport, and orientation.
- [x] Apply validated selection and presentation commands on the GPUI thread
  through a bounded queue with bounded response deadlines.
- [x] Add Designer MCP tools for preview status, selection mode, and viewport
  updates.
- [x] Show the connected preview, selected source link, viewport presets, and
  rotation controls in Designer.
- [x] Test RPC validation, queue bounds, and stable source-location reporting.
- [x] Document that this ticket controls the one mounted entry and does not
  claim screen switching or visual zoom.

### T015-8 — Compile the folder-based Studio source project

- [x] Define optional `studio.project.json` screen entries with stable IDs and
  project-relative `.studio` source paths.
- [x] Validate schema version, entry count, unique IDs, normalized paths,
  canonical root containment, file type, and source-size limits.
- [x] Compile the declared source files into one in-memory IR module in
  declaration order; retain the current one-entry and AssemblyScript fallback
  when no project descriptor is present.
- [x] Hash the descriptor and all declared sources so add/edit/delete changes
  trigger hot reload; do not create a `.studio` archive during development.
- [x] Preserve each component's source path for selection/source links.
- [x] Test descriptor validation, legacy fallback, source graph changes,
  component identities, and source-map paths.

### T015-9 — Add multi-screen routing to the dev preview

- [x] Preserve the selected screen across source reloads when its ID remains
  declared, and fall back deterministically when it is removed.
- [x] Switch the mounted screen through the bounded dev RPC and return the
  active screen identity and source map.
- [x] Add Designer controls for the declared screen IDs and selected-screen
  source links.
- [x] Test route selection, state retention per screen, hot reload, and source
  links for multiple screen files.

### T015-10 — Add visual zoom to the dev preview

- [x] Scale native preview layout dimensions and text metrics before GPUI
  layout so generated hitboxes stay aligned with the rendered controls.
- [x] Apply zoom updates through the bounded dev RPC with validated limits.
- [x] Add Designer zoom controls and report the applied zoom in preview status.
- [x] Test zoom bounds, visible geometry at multiple scales, and pointer hit
  targets at scaled coordinates.
