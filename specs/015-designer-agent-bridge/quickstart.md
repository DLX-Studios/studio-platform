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
the last applied revision or the latest reload diagnostic.

The file tools operate on project-relative UTF-8 source files. New files can be
created inside existing project directories. They reject path escapes,
secret files, and generated/dependency/cache directories. Binary asset editing,
Designer-window source-buffer synchronization, component-to-source selection,
and screen selection are later tickets; this first bridge writes the same source
tree that `studio dev` compiles.
