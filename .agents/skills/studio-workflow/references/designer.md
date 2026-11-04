# Designer source editing

`project.list_files` lists permitted UTF-8 source files and SHA-256 revisions.
`project.search` takes a single-line literal `query` and optional
`max_results` (1–200, default 50). Results contain path, revision, one-based
line/character column, and zero-based UTF-8 byte `start`/`end` ranges; a
`truncated` flag means more matches exist. Generated files, secrets, dependency
directories, binary assets, and paths outside the canonical root are excluded.

Read the target with `project.read_file` before editing. Use
`project.apply_patch` for focused replacements:

```json
{
  "path": "app.studio",
  "expected_revision": "sha256:the-revision-returned-by-read-file",
  "edits": [{ "start": 6, "end": 11, "replacement": "Welcome" }]
}
```

Ranges refer to the original file's UTF-8 bytes, with an exclusive end. They
must fall on character boundaries and cannot overlap. `project.write_file`
accepts `path`, `content`, and `expected_revision`; supply null for create-only.
Parent directories must already exist. File writes are atomic and bounded to
4 MiB. Tool failures use `isError: true` and `structuredContent.error`.

Successful writes publish local source events with client attribution. If the
Designer GUI has this same source folder linked to an open project, clean
buffers refresh and dirty buffers retain a conflict for reconciliation.
Running the MCP server does not open or authenticate the Designer GUI.

Use `preview.status` for the mounted screen, available screen IDs, selection
and its source location, applied revision, presentation, and last diagnostic.
`preview.selection.set_mode` with `{"enabled": true}` enables user hit
selection. `preview.presentation.set` accepts any combination of `screen_id`,
`viewport: {width, height}`, `orientation`, and `zoom_milli`:

```json
{
  "screen_id": "home",
  "viewport": { "width": 390, "height": 844 },
  "orientation": "portrait",
  "zoom_milli": 1000
}
```

Dimensions must be 320–8192 logical pixels and agree with orientation when
both are provided; zoom is 250–4000 thousandths. Screen IDs come from the
project's `studio.project.json`. Read the selected node's source file to edit
its code. These tools do not provide screenshots, arbitrary node clicks, or
release runtime UI inspection.
