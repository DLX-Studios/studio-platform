# Agent-to-Designer quickstart

## Native Designer and preview windows

Open an authenticated Designer project, then choose **Open folder…** in its
project toolbar. Select the application's source folder. Designer saves this
association and invokes `studio dev <folder>`, which opens a separate native
preview window. On subsequent project opens, use **Open preview…** to start it.
The folder picker is also available with Ctrl+O (Command+O on macOS) inside the
project. Resolve or save a dirty source buffer before choosing another folder.

The toolbar reports startup and live preview readiness separately. **Restart
preview** resets the guest through `studio restart-session`; **Stop preview**
only closes a process started by this Designer project window. A preview started
from a terminal or MCP remains owned by that launcher. Its status and controls
still work through the existing local JSON-RPC endpoint. Project closure or
folder replacement disposes the preview and tasks owned by the Designer view.

**Check** validates saved `.studio` source files and **Build** runs the existing
CLI release packaging pipeline. Check, build, and restart share one task slot;
use **Cancel task** to interrupt it. **Output** shows bounded command diagnostics
and automatically opens on failure. Multi-screen development support does not
extend the CLI's existing release-packaging capabilities.

Place `studio` beside `studio-designer` or on PATH. A host-level
`STUDIO_CLI_BINARY` override can select a specific executable. Native previews
inherit Designer's graphical session environment. The two windows exchange
status, selection/source links, screens, viewport, and zoom through JSON-RPC 2.0
over the project's local dev socket; source files remain the editable state.

## Coding agent setup

The agent server is launched by the agent host as a local MCP process. It uses
JSON-RPC 2.0 over newline-delimited stdio and is part of the `studio-designer`
executable; it does not need Bun or Node.

```sh
cd /path/to/app
studio-designer agent-server
```

For Codex, add a local server to `~/.codex/config.toml`:

```toml
[mcp_servers.studio_designer]
command = "studio-designer"
args = ["agent-server"]
```

For OpenCode, add a local server to `opencode.json` in the project:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "studio-designer": {
      "type": "local",
      "command": ["studio-designer", "agent-server"],
      "enabled": true
    }
  }
}
```

The agent host should launch the server in the app folder. Verify that
`project.info.project_root` matches that folder before editing. A fixed
`--project-root PATH` is optional; portable configs omit it and `cwd`.
The complete tool catalog, asynchronous CLI/runtime operations, lifecycle
semantics, and reusable skill are documented in
[Studio agent tools](../../docs/agents/README.md).

An agent should read a file first, keep its returned `revision`, then pass that
revision to `project.write_file` or `project.apply_patch`. A stale revision
means the file changed since it was read; the agent should reread and reconcile
instead of overwriting. `studio dev <project-folder>` reads and watches the
same editable project folder and applies successful edits to its running
preview; it does not load a compressed `.studio` release archive.
`preview.status` reports
the last applied revision, selected node and source span, viewport, and latest
reload diagnostic. `preview.selection.set_mode` enables preview hit selection;
`preview.presentation.set` updates viewport, orientation, declared screen, and
zoom.

The file tools operate on project-relative UTF-8 source files. New files can be
created inside existing project directories. They reject path escapes,
secret files, and generated/dependency/cache directories. They do not edit
binary assets. In Designer, link the same local
project folder to the authenticated project to open source files, see agent
updates, resolve unsaved conflicts, and use revision-checked saves or reverts.
Designer can also enable preview selection, apply phone/tablet/desktop viewport
presets, rotate the viewport, switch declared screens, adjust zoom, and open the
active screen or a selected node's `.studio` source location.

For projects with more than one `.studio` screen, the folder can declare the
source graph in `studio.project.json`:

```json
{
  "schemaVersion": 1,
  "screens": [
    { "id": "home", "source": "app.studio" },
    { "id": "settings", "source": "screens/settings.studio" }
  ]
}
```
