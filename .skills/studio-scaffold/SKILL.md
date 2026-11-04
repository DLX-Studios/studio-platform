---
name: studio-scaffold
description: "Create a runnable Studio Platform app or plugin with studio new, manifest.json, and Svelte-like app.studio source. Use for a new standalone app, an existing empty app folder, or the Studio scaffold step inside an agreed workspace layout."
---

# Studio app scaffold

Create the app in the folder the user chose, using the installed Studio CLI
and its template gallery. Studio Script uses Svelte-like `.studio` source and
native Studio components. A new app contains guest source and its manifest;
the shared `studio-app` executable runs the eventual release bundle.

## Choose the destination and template

Read applicable project guidance and inspect the destination first. Preserve
an established repository layout. If whole-project structure is unresolved,
use an available `studio-init` / `studio-project-init` workflow; for a nested
app, retain the placement and app-brief requirements from `studio-app-init`.
Do not require a multi-app layout, Console, or new design process just to
create a standalone Studio app.

Use `studio new --help` and `studio new --list-templates --format json` to
discover the installed CLI. Default to `blank` unless the requested workflow
fits another template. Ask only for missing name, destination, or template
choices that materially affect the scaffold. Record requested delivery targets
without treating a successful local scaffold as verification of every platform.

Read [scaffolding details](references/scaffolding.md) before running the command;
it covers the destination rules, existing folders, and actual self-check.
Run `studio new <slug> --template <template>` from the chosen parent directory.
For a user already running an agent inside a folder, keep that folder as the
destination; avoid accidentally creating another nested app with the same name.

## Make the scaffold usable

Inspect the generated `manifest.json` and `app.studio`. Keep the generated
schema/entry/SDK fields valid, give the app its intended identity, and request
only the capabilities it needs. Implement feature code using `studio-code`
when available. Do not add a JavaScript web scaffold or handwritten
AssemblyScript as a substitute for the modern Studio Script template.

If app-specific agent guidance is needed, record the actual source files,
commands, and matching skills in its `AGENTS.md`. Preserve existing guidance
and inherit workspace conventions; create `APP.md` when that workspace calls
for it. The CLI itself does not generate either file.

## Check and develop

The scaffold command parses/validates/lowers `app.studio` before publishing it.
It does not run a release build. Validate any subsequent source edits with
`studio check <app-folder>/app.studio`, then use `studio dev <app-folder>`
for a native preview with hot reload. For declared multiple screens, check
each source and keep `studio.project.json` consistent.

When MCP is available, confirm `project.info.project_root` equals the app
folder before editing or launching. Use `preview.start`, then inspect
`preview.status` for readiness and `preview.logs` for failures. `studio.check`
is asynchronous: inspect `studio.task.status` and `studio.task.logs`. A shell
`cd` does not retarget an MCP server that is already running. Read
`studio-workflow` if available for server setup and complete lifecycle details.

Report the app path, selected template, files created, checks actually run,
and whether a preview was opened. Use `studio build <app-folder>` only when a
release bundle is needed; report compiler or publisher-trust prerequisites
instead of claiming a source check proves release readiness.
