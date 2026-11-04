---
name: studio-workflow
description: "Configure and operate Studio Designer MCP tools, asynchronous CLI jobs, native dev previews, and release runtime processes. Use for agent tooling setup, lifecycle, and diagnostics; studio-scaffold covers new apps and studio-code covers app source authoring."
---

# Studio workflow

Use one local `studio-designer agent-server` process for source editing, CLI
tasks, preview controls, and release process management. The Designer GUI is
optional. Saved source files are the shared state observed by Designer and
`studio dev`; never edit lowered IR or rebuild a package for each preview edit.

For new app scaffolds use an available `studio-scaffold` skill; for writing
Svelte-like Studio Script source use `studio-code`. This skill covers the
shared tool setup and process workflow used by both.

Call `project.info` before edits or launches and confirm its canonical
`project_root` matches the user's app folder. With no `--project-root`, the
server uses its startup working directory. A mismatch means fix the launch
configuration before writing; changing the agent's shell directory does not
retarget an already running server.

Read only the reference relevant to the work:

- [Setup](references/setup.md): install/discover the binaries and skill, register
  one MCP server, and use the portable Codex/OpenCode config assets.
- [Designer editing](references/designer.md): find source, patch with revisions,
  reconcile conflicts, and inspect screen/selection source links.
- [CLI and runtime](references/cli-runtime.md): start the native preview, check
  or build asynchronously, inspect diagnostics, and launch release bundles.

Discover the current tool schemas using MCP `tools/list`; reference examples
describe the shipped workflow but the server defines the accepted arguments.
Use `project.read_file` revisions for existing files and `expected_revision:
null` only when creating files. On a revision conflict, reread and reconcile.

Distinguish process startup from preview readiness: `preview.start` returns
after spawning; `preview.status` reports whether compilation and application
succeeded. Inspect diagnostics on failure instead of repeatedly relaunching.
For automated smoke tests, stop the processes you started when finished;
preserve existing previews and source edits that belong to the user.
