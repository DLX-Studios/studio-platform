---
name: studio-code
description: "Write and modify Studio Platform app code in Svelte-like .studio files using the supported reactive subset and native component catalog. Use for app UI, state, handlers, screens, and the Designer MCP edit/check/hot-reload workflow; excludes changes to the Rust host itself."
---

# Write Studio app code

Studio Script looks like Svelte: `<script lang="ts">`, reactive runes,
interpolation, conditional/each blocks, and event handlers. Source files use
`.studio`. The `rsvelte` frontend parses them; Studio validates a portable
subset and lowers it into typed IR for native rendering. The release path
lowers to AssemblyScript/Wasm. Parsing Svelte-shaped syntax does not make
arbitrary JavaScript, browser APIs, npm modules, or SvelteKit features runnable.

## Establish the app context

Read the app's `AGENTS.md`, manifest, source entry, declared screens, and
existing examples. Follow applicable design guidance. Scaffold a missing app
with `studio-scaffold` if available. Preserve the app's source organization;
avoid introducing another framework, bundle format, or new host capability
merely to implement a UI feature.

Read [Studio Script authoring](references/studio-script.md) before choosing
syntax, components, or SDK calls. It includes a runnable counter and the
compiler boundaries that differ from ordinary Svelte. Prefer the installed
compiler and current fixtures over stale roadmap or transform-plan prose.

Use the [component index](references/components.md) to find each catalog
component's props, child shape, events/state, support limits, and checked
example. Start with [Box](references/components/layout.md#box) for the general
div-like wrapper, then Row/Column/Grid for deliberate layout. Read only the
family needed; generic catalog surfaces do not imply full upstream widget
behavior. Shared [contracts and reactive examples](references/components/contracts.md)
explain prop values and event binding limits.

## Edit source and observe the preview

When Designer MCP tools are available:

1. Call `project.info`; verify the canonical root is the app folder. Discover
   actual tool schemas. A shell directory change cannot retarget this server.
2. Find code with `project.list_files` / `project.search`, then read it with
   `project.read_file`. Keep the returned SHA-256 revision.
3. Apply a focused `project.apply_patch`, or `project.write_file` with that
   expected revision. Ranges are zero-based UTF-8 bytes with an exclusive end.
   Use null only for create-only writes; reread and reconcile stale revisions.
4. Start a missing dev preview with `preview.start`. After saved edits,
   inspect `preview.status` for the applied revision/hash, phase, and latest
   diagnostic. Read `preview.logs` for process/compile startup failures.
5. Validate sources with `studio.check`; poll `studio.task.status` and read
   `studio.task.logs`. Check/build/restart share one asynchronous task slot.
   Spawn success is not validation success or proof of a rendered window.

With shell tools, edit the same source files, use `studio check <source-file>`
(all changed screens as appropriate), and run `studio dev <app-folder>`.
Do not rebuild a release archive for each edit. Hot reload watches source,
compiles changed inputs, and preserves the previous live revision on failure.
Source changes can also appear in an open Designer project linked to this
folder; the GUI is optional for the MCP workflow.

## Inspect presentation and ship deliberately

Use `preview.presentation.set` for a declared `screen_id`, logical viewport,
orientation, or zoom. Use `preview.selection.set_mode` to enable user hit
selection, then inspect selection/source links with `preview.status`. These
tools do not provide screenshots or arbitrary automated node clicks.

Use `studio.build` or `studio build <app-folder>` when a release artifact is
requested. Read task completion/diagnostics before launching it through
`runtime.start` / `studio-app --bundle`. Release tools manage the pure runtime
process; editing, selection, and hot reload use the dev host. Stop only the
processes started for the task. An available `studio-workflow` skill contains
the complete lifecycle, config, and log semantics.

Report the source change, compiler/preview evidence, and any unverified
behavior. A source check is not an interaction test or release packaging test.
