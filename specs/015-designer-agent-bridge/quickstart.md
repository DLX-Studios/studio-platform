# Agent-to-Designer quickstart

The agent server is launched by the agent host as a local MCP process. It uses
JSON-RPC 2.0 over newline-delimited stdio and is part of the `studio-designer`
executable; it does not need Bun or Node.

```sh
studio-designer agent-server --project-root /absolute/path/to/project
```

For Codex, add a local server to `~/.codex/config.toml`:

```toml
[mcp_servers.studio_designer]
command = "studio-designer"
args = ["agent-server", "--project-root", "/absolute/path/to/project"]
```

For OpenCode, add a local server to `opencode.json` in the project:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "studio-designer": {
        "type": "local",
        "command": [
          "studio-designer",
          "agent-server",
          "--project-root",
          "/absolute/path/to/project"
        ]
      }
    }
  }
}
```

An agent should read a file first, keep its returned `revision`, then pass that
revision to `project.write_file` or `project.apply_patch`. A stale revision
means the file changed since it was read; the agent should reread and reconcile
instead of overwriting. `studio dev <project>` watches the same project root
and applies successful edits to its running preview. `preview.status` reports
the last applied revision, selected node and source span, viewport, and latest
reload diagnostic. `preview.selection.set_mode` enables preview hit selection;
`preview.presentation.set` updates viewport and orientation.

The file tools operate on project-relative UTF-8 source files. New files can be
created inside existing project directories. They reject path escapes,
secret files, and generated/dependency/cache directories. They do not edit
binary assets. In Designer, link the same local
project folder to the authenticated project to open source files, see agent
updates, resolve unsaved conflicts, and use revision-checked saves or reverts.
Designer can also enable preview selection, apply phone/tablet/desktop viewport
presets, rotate the viewport, and open a selected `.studio` source location.

The current `studio dev` compiler mounts one `.studio` entry per session, so
screen switching is not available. The RPC reports the default zoom value and
rejects non-default zoom changes until the native preview renderer can scale
the rendered tree and input targets together.
