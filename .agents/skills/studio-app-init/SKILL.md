---
name: studio-app-init
description: "Add or scaffold one runnable app inside an established multi-app or platform workspace. Use when creating an app under /apps/{app_name}; use studio-init for whole-project structure decisions."
---

# Studio App Init

Add one app to an existing multi-app/platform workspace while following that repository's decisions. The default app profile is a Studio Platform app/plugin scaffolded with the current `studio new` workflow. Let the project manager choose another implementation when the product or its release requirements call for it.

Before asking the project manager to choose app type, platform, or scaffold, read and follow [visual interaction for Studio planning](references/visual-interaction.md). Use the host's structured controls or a visual choice board with platform support labels when available; retain a concise text equivalent.

## Inspect the workspace

- Read root `AGENTS.md`, `STUDIO.md`, workspace manifests, and a few existing app directories before proposing changes.
- Check the multi-app workspace root for `DESIGN.md`. If present, use it as the shared design source and include its path in the app brief; do not copy it into the nested app. If absent, call `studio-design` before scaffolding and pass the app purpose plus any appearance preferences already collected by `studio-init`. When invoked directly, ask only for missing design context.
- Confirm that the project already chose a multi-app/platform layout. If its root structure is unresolved, use `studio-init` first instead of deciding it here.
- Locate the Studio Platform repository or installed `studio` CLI from the workspace context; do not assume a fixed machine path. When available, inspect its current `AGENTS.md`, `README.md`, `studio new --help`, template gallery, and `specs/011-studio-new/spec.md`. Follow the actual CLI and current gallery over stale examples.
- Clarify the new app's purpose, users, main workflow, app name/slug, primary application type, delivery surfaces, selected target platforms, runtime/framework, data ownership, and integration needs. Ask explicitly whether the app must run in a browser, be installed on mobile/desktop devices, or do both. Do not infer native installation from responsive layouts or infer web-only from a request to work on phones, tablets, and computers. Ask only questions whose answers change its boundary or scaffold.

## Offer the app type

Offer **Studio Platform app/plugin** as the recommended default for app creation, including when the user selects all delivery surfaces. It is a modern Studio Script app described by `manifest.json` and `app.studio`, scaffolded from the Studio Platform template gallery, and run by the existing native `studio-app` host. It is not a request to copy or add the `studio-app` host binary into the new app.

Keep this default when a requested target is not supported yet: mark the target with its truthful support status, explain the Studio Platform work needed to reach it, and ask whether that surface is a v1 release requirement or an accepted roadmap target when the distinction affects the scaffold. Do not silently replace the Studio Platform default with SvelteKit + Tauri just because the user selected every surface.

Use the current gallery and `studio new --list-templates` when available. The CLI's blank template is the general starting point; choose a more specific gallery template only when its described layout fits the app's actual workflow. The current `studio new` design generates modern Studio Script projects, not AssemblyScript projects. If the platform or CLI version differs, inspect its current instructions before relying on these details.

Show the user the platform targets alongside each matching type, then record the app's selected `target_platforms` separately from its type, language, and framework. Use support labels (supported, limited/experimental, unverified), and verify them against the current framework and host documentation. The root `STUDIO.md` has the initial target matrix and specific qualifications for Studio Platform and GPUI Kit.

Offer **SvelteKit + Tauri** as an alternate implementation when the user explicitly prefers a standalone web/native stack or requires browser and installed releases before the Studio Platform host supports all selected surfaces. It can share SvelteKit UI and app logic across browser, desktop, and mobile, but needs separate build outputs: use the project's SvelteKit server adapter for the web deployment (normally `@sveltejs/adapter-node`) and a static/SPA build for Tauri. Tauri does not host SvelteKit SSR; use a client/server API boundary for server-backed features. Do not imply one adapter build delivers both SSR website and native packages. Verify the current Tauri/SvelteKit guide, selected plugins, and build prerequisites for every chosen platform; iOS builds require macOS and Xcode.

Then offer the narrower delivery choices in this order, when relevant:

1. **SvelteKit web app only:** browser access on desktop, tablet, and phone; no native mobile/desktop package.
2. **Installed mobile + desktop app:** Android/iOS plus Windows/macOS/Linux; no browser-delivered app. Tauri with Svelte is an option when its current target and feature support fits.
3. **Installed mobile-only app:** Android/iOS; no browser app or desktop package.
4. **Installed desktop-only app:** Windows/macOS/Linux; no browser app or mobile package. Keep the dedicated GPUI Kit option below visible here.

When the user asks for an installed app, do not continue with a responsive web app as though that meets the request. Ask whether they also want browser access and display the relevant delivery choices with explicit **Browser**, **Installed desktop**, and **Installed mobile** columns. A request for multiple targets is valid; have the user select the desired surfaces instead of forcing one OS family.

Make combinations easy to choose without requiring the user to type every target. If the host supports multi-select, offer the three surface groups as checkboxes plus **Select all** and **Custom**. Otherwise provide quick choices for **All surfaces** (browser + installed desktop + installed mobile), **Browser + installed mobile**, **Browser + installed desktop**, **Installed mobile + desktop only**, and the applicable single-surface options. Map replies such as “all of the above,” “both,” or “desktop too” to the choices just shown. “Both” means the two explicitly contrasted options; if the intended pair is unclear, ask one short follow-up. Summarize the mapped channels before selecting the implementation profile. Recommend Studio Platform for the **All surfaces** profile by default, while separately reporting which requested channels its current host actually supports.

The project's shared application types also include Web app/site, Desktop app, Mobile app, CLI tool, Backend/API service, Background worker, and Firmware/embedded app. Use the best matching type if the user selects a different implementation style; add a shared type only when none fits. Give each nested app one primary type by default and add secondary types only for a genuinely hybrid app. A project's multi-select type metadata can summarize the types of its apps.

For the combined SvelteKit + Tauri profile, use `Web app/site` as the primary catalog type by default and record `SvelteKit` / `Tauri` in implementation metadata. Record every agreed browser and installed OS target in `target_platforms` with support status and current evidence. If the app has meaningful independent web, desktop, and mobile product boundaries, discuss whether separate app entries are clearer than one hybrid app.

Offer **GPUI Kit Rust desktop app** as a dedicated implementation choice under **Desktop app**: it is a pure Rust desktop application using GPUI Kit, with Windows, macOS, and Linux as its documented desktop targets. Save its catalog application type as `Desktop app` and record `Rust` / `GPUI Kit` as implementation metadata. Do not imply that its mobile path is equivalent desktop/full-app support: current GPUI Kit docs call mobile experimental and describe limited component embedding in native applications. If mobile is requested, explain the native-host requirement and validate the current integration before planning it as a target.

## Plan and scaffold

- Put the app at `/apps/{app_name}`. Use a lowercase, filesystem-safe slug consistent with neighboring app folders and the selected scaffold's naming rules.
- For a Studio Platform app, use the available `studio-scaffold` skill for the CLI step, passing the approved `apps/` parent, slug, template, and agreed context. Otherwise use the supported `studio new <slug> [-t <template>]` command from that parent. Its destination is based on the CLI's working directory. Use `studio-code` when available for the resulting Svelte-like Studio Script source and edit/check/hot-reload loop.
- Never pass a force/overwrite option over an existing app. Inspect the destination first; if it contains files, preserve them and agree on a different name or an explicit migration plan.
- Ensure every nested app has `/apps/{app_name}/APP.md` and `/apps/{app_name}/AGENTS.md`. `APP.md` is the canonical, version-controlled app brief for the Console catalog; follow the YAML front matter schema in root `STUDIO.md`. Only record agreed decisions and verified current facts. Preserve and reconcile an existing brief rather than overwriting it.
- Create or update app `AGENTS.md` after scaffolding; if one exists, preserve useful guidance. Link to `APP.md` and inherited workspace files. Include the code map, actual run/build/check/test commands, conventions and constraints from applicable specs, and matching available skills with when to use them. Discover and list skills from the current agent's catalog, use `$find-skills` when needed to search, and read/follow the relevant matches for the work. Refer to skills by name, never by machine-specific external paths. Do not assume another agent has the same skills installed or install skills unless requested. Do not copy whole specs or invent commands. Keep the file concise and workflow-specific.
- Update `APP.md` when app identity fields or linked specs change; update `AGENTS.md` when stack guidance, commands, spec-driven constraints, or matching workflow skills change. Keep their overlapping details aligned. The Console may index/cache `APP.md`; canonical metadata edits must update the brief in the same change rather than silently diverging in the database.
- For a Studio Platform app, `specs/011-studio-new/spec.md` defines a modern Studio Script scaffold and describes source/build validation. The current CLI implementation parses/validates/lowers the staged `app.studio` before publishing; it does not run a release build as part of `studio new`. Perform any required release validation explicitly and distinguish it from the scaffold check. The CLI does not generate `APP.md` or `AGENTS.md`, so create both for this nested-app workflow afterward. Include only template- and app-relevant requirements from the current spec/gallery.
- Keep the Studio app's own identity and required metadata in its manifest. Request only capabilities the app actually needs. Keep credentials out of guest state and messages; use host-managed secrets and opaque authorization references described by the platform.
- For a non-Studio app, use the selected type's existing scaffold, runtime, and applicable skill. Keep native manifests (`package.json`, `Cargo.toml`, or equivalent) at the app root. For a web console, follow root `STUDIO.md` for SvelteKit, Bun/SQLite, workspace `DESIGN.md` tokens, components, and `/admin` security.
- If this nested app is the selected project Console, route its implementation to `studio-console-init`, passing the agreed slug, workspace placement, and design-file context. Do not scaffold the Console with the generic Studio plugin template.
- Add root workspace configuration, shared packages/crates, or cross-app wiring only when the new app needs them. Do not create a package, crate, or folder solely to match a template tree.

Use a concise app brief before scaffolding when the app has significant data, security, deployment, or cross-app boundaries. Describe the chosen app type/template, path, interfaces, shared dependencies, and open decisions. For a routine app with clear requirements, make the scaffold directly and summarize those choices afterward.

## Integrate and hand off

Update only the workspace manifests, app catalog, root `STUDIO.md`, README, or agent guidance affected by the new app. Preserve the whole-project decisions and avoid unrelated restructuring. Prefer the Studio CLI's own template validation; report precisely which checks it performed and any additional commands run. Do not claim the app was launched or runtime-tested unless that happened.

Report the new app path, selected type/template, workspace integration, documentation changes, validation evidence, and unresolved decisions. Use `studio-commit` for a separate checkpoint when requested.
