# Designer Agent Bridge

## Goal

Let a local coding agent make reviewable changes to a Studio project through a
Designer-owned API. Designer reads and writes the project's source files with
revision checks. The existing `studio dev` file watcher compiles and applies
successful edits, while Designer can query the active preview for its status.

## First milestone

- The agent starts `studio-designer agent-server --project-root <path>` as a
  local MCP server using JSON-RPC 2.0 over newline-delimited stdio.
- The server exposes project file listing, reading, full-file writing, and
  UTF-8 range patches. Writes require the SHA-256 revision the agent read.
- Project paths are relative, bounded, confined to the selected root, and
  exclude generated files and build/dependency directories. Writes are atomic.
- `studio dev` keeps owning the preview window and watching the project tree.
  Designer writes to the same source tree; no IR is persisted as source.
- A separate local JSON-RPC endpoint on `studio dev` reports the latest applied
  revision and the most recent reload diagnostic. The MCP tool relays this
  status when a preview is running.
- Existing `studio-reload/1` clients and the production runtime remain
  compatible.

## Out of scope for this milestone

- Translating arbitrary IR mutations back into source code.
- A new full source editor or local-project catalog inside the Designer window.
- Component selection and screen navigation events from the preview back into
  Designer. Those will build on the same session protocol after source writes
  and preview status work end to end.
- Remote project access or network listeners. Agent access is local to the
  configured stdio process and project root.

## Acceptance

An MCP client can list and read project sources, apply a hash-guarded patch or
write a full file, and receive a new content revision. A concurrently changed
file causes a conflict instead of an overwrite. When `studio dev` is running
for that root, it detects the write and its local status endpoint eventually
reports the applied revision; failed compilation leaves the previous preview
live and returns a diagnostic through status.

## Folder-based development sources

`studio dev <project-folder>` uses the folder as the editable source project.
The `.studio` ZIP archive is a build/release artifact and is not the source
mounted by dev. An optional root `studio.project.json` declares ordered screen
entries and their stable IDs:

```json
{
  "schemaVersion": 1,
  "screens": [
    { "id": "home", "source": "app.studio" },
    { "id": "settings", "source": "screens/settings.studio" }
  ]
}
```

Every source path is normalized, project-relative, canonicalized within the
project folder, and points to a bounded `.studio` text file. Without the
descriptor, existing single-entry `.studio` and AssemblyScript projects keep
their current behavior. Changes to the descriptor or declared sources trigger
the in-memory dev compiler without creating a `.studio` archive.
