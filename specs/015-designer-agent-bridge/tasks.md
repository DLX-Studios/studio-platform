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

- Associate a local source root with an authenticated Designer project.
- Refresh open text buffers after agent writes and preserve unsaved edits as
  explicit conflicts instead of silently replacing them.
- Show file diffs, agent attribution, and undo/revert actions in Designer.

### T015-7 — Bridge preview selection and presentation controls

- Report selected component IDs and screen IDs from the preview to Designer.
- Let Designer change the active screen, viewport, orientation, and zoom over
  the Studio dev JSON-RPC session.
- Preserve source links where the compiler can map rendered components back to
  files and spans.
