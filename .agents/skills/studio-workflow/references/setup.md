# Local MCP setup

Install `studio-designer`, `studio`, and `studio-app` on PATH, or keep the
three executables together. The server resolves CLI/runtime binaries from
`STUDIO_CLI_BINARY` / `STUDIO_RUNTIME_BINARY` when supplied by the host,
otherwise beside itself, then from PATH. Tool arguments cannot select an
arbitrary executable or project root.

In a platform checkout, build the binaries using its Cargo target configuration:

```sh
cargo build --locked -p studio-designer -p studio-cli -p studio-app
```

Place that target's `debug` directory on PATH for development. Native platform
libraries are still required. MCP itself needs no graphical session or Node;
opening a preview/runtime needs a display, and building Wasm requires the
existing AssemblyScript toolchain. MCP does not install compilers or bypass
release publisher trust.

Register once in Codex:

```sh
codex mcp add studio_designer -- studio-designer agent-server
```

Alternatively merge [the Codex TOML asset](../assets/codex.toml) into
`~/.codex/config.toml`. Do not replace unrelated settings. For OpenCode, merge
[the OpenCode JSON asset](../assets/opencode.json) into its config; `mcp` maps
server names directly, without a `servers` wrapper. Official formats:
[Codex MCP](https://learn.chatgpt.com/docs/extend/mcp) and
[OpenCode MCP](https://opencode.ai/docs/mcp-servers/).

Both assets omit `cwd` and `--project-root`. Start the agent in the intended
app folder, then confirm `project.info.project_root` before making changes.
The server uses the working directory supplied by the host; it cannot infer a
different editor workspace. If a host launches from another folder, fix that
host's launch context or deliberately use `--project-root PATH` for a fixed
workspace. There is no Studio-specific `${workspaceFolder}` substitution.

Install this self-contained skill for use outside the platform checkout by
copying its complete folder to `~/.codex/skills/studio-workflow` (or
`$CODEX_HOME/skills/studio-workflow` when set). A local symlink to this folder
also works. Restart the agent session to discover new skills/MCP tools.
The platform checkout's `.agents/skills/studio-workflow` is the maintained copy.

For a new app, first scaffold with `studio new my-app`, then enter `my-app`
and launch the agent. An empty folder can host the MCP server and receive
source files, but preview/build need a valid manifest and source entry.
