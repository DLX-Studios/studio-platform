# Studio agent tools

Agents use one local MCP server for Designer source edits, Studio CLI tasks,
native development previews, and release runtime processes:

```sh
studio-designer agent-server
```

The server defaults to its startup folder. Register it once, then start Codex
or OpenCode from each app's folder. Confirm `project.info` points to that folder
before editing. The Designer GUI can be opened separately to review source
changes; it does not have to be running for MCP tools to work.

## Setup

Make `studio-designer`, `studio`, and `studio-app` available on PATH, or place
them together. Register with Codex:

```sh
codex mcp add studio_designer -- studio-designer agent-server
```

For manual configuration, merge the maintained examples into your existing
config:

- [Codex TOML](../../.agents/skills/studio-workflow/assets/codex.toml)
- [OpenCode JSON](../../.agents/skills/studio-workflow/assets/opencode.json)

The examples omit a fixed `cwd` and `--project-root`. Hosts must supply the app
folder as the server working directory. An explicit
`--project-root /path/to/app` remains available for a deliberately fixed server.
See [setup and skill installation](../../.agents/skills/studio-workflow/references/setup.md)
for binary resolution, dependencies, and host configuration details. The
examples follow the official [Codex](https://learn.chatgpt.com/docs/extend/mcp)
and [OpenCode](https://opencode.ai/docs/mcp-servers/) MCP formats.

## Tools

| Area | Tools | Details |
| --- | --- | --- |
| Designer source | `project.info`, `project.list_files`, `project.search`, `project.read_file`, `project.write_file`, `project.apply_patch` | [Revision checks, byte ranges, source links](../../.agents/skills/studio-workflow/references/designer.md) |
| Dev preview | `preview.start`, `preview.status`, `preview.logs`, `preview.stop`, `preview.restart`, `preview.selection.set_mode`, `preview.presentation.set` | [Lifecycle and presentation](../../.agents/skills/studio-workflow/references/cli-runtime.md) |
| CLI | `studio.check`, `studio.build`, `studio.task.status`, `studio.task.logs`, `studio.task.cancel` | [Asynchronous commands and diagnostics](../../.agents/skills/studio-workflow/references/cli-runtime.md) |
| Release runtime | `runtime.start`, `runtime.status`, `runtime.logs`, `runtime.stop` | [Bundle launch and trust requirements](../../.agents/skills/studio-workflow/references/cli-runtime.md) |

The runtime tools launch the existing pure `studio-app` executable. They add
no agent server or development mode to release builds. Process status reports
process liveness; dev RPC reports compilation state. UI screenshots and release
runtime inspection are not exposed.

## Example workflow

Use the focused skills for the work at hand:

| Skill | Use |
| --- | --- |
| [studio-scaffold](../../.skills/studio-scaffold/SKILL.md) | Create an app with `studio new`, including an agent's existing destination folder. |
| [studio-code](../../.skills/studio-code/SKILL.md) | Write supported Svelte-like `.studio` syntax using the [100-component reference](../../.skills/studio-code/references/components.md), then develop through source edits, checks, and hot reload. |
| [studio-workflow](../../.agents/skills/studio-workflow/SKILL.md) | Configure MCP and manage CLI tasks, previews, and release runtime processes. |

The new app-authoring skills are maintained under `.skills/`. Discovery links
under `.agents/skills/` and the user skills directory point to those same
folders, so there is one maintained copy of each.

1. Scaffold an app with `studio new my-app`, enter its folder, and start the
   agent. Confirm `project.info` returns this folder.
2. Call `preview.start`, then poll `preview.status` until ready. Read
   `preview.logs` if startup or compilation fails.
3. Find and read a source file, then apply an edit with its returned revision.
   Watch `preview.status` for the applied revision or a reload diagnostic.
4. Call `studio.check`, poll `studio.task.status`, and read `studio.task.logs`
   for validation diagnostics. Use `studio.build` when a release bundle is
   needed; it follows the same asynchronous task pattern.
5. With publisher trust provisioned, use `runtime.start` with the
   project-relative bundle path. Inspect `runtime.status` and `runtime.logs`.
6. Stop the processes started for the task. On normal MCP closure, the server
   also stops its managed children. Externally started previews remain owned
   by their original launcher.

The reusable [studio-workflow skill](../../.agents/skills/studio-workflow/SKILL.md)
contains the agent guidance and portable MCP assets. Keep this server as a
single registration; separate registrations for CLI/runtime would create
independent process ownership and duplicate the tool list.
