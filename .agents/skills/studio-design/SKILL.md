---
name: studio-design
description: "Create or refine a project's root DESIGN.md from its agreed visual direction. Use when initializing a project with no design guide or when the user asks to shape or change its design system."
---

# Studio Design

Create or refine the project's durable visual system in `DESIGN.md`. Use this skill from `studio-project-init`, `studio-app-init`, or `studio-console-init` when the project root has no design guide, or directly when the user requests design-system work.

Before asking appearance questions, read and follow [visual interaction for Studio planning](references/visual-interaction.md). Use visual references, swatches, layout sketches, or available native choice controls to help the user compare directions; do not rely on text-only color or style descriptions when the host can show a suitable visual.

## Gather and route design context

- Inspect the project root, `AGENTS.md`, `STUDIO.md`, existing `DESIGN.md`, application styles, installed component libraries, and any user-provided visual references.
- Reuse design and appearance decisions already collected by `studio-init` or `studio-app-init`; do not ask the user to repeat them. If context was not passed, ask a small set of high-impact questions about audience and purpose, desired mood, references, color preferences, typography, layout density, and accessibility needs. Offer a reasoned starting direction when details are open.
- Treat the project-root `DESIGN.md` as the shared visual source for apps in a multi-app workspace. If it exists, use it by default and update it only when the user or agreed project direction calls for a design change. Do not make duplicate app-level guides unless the workspace already has an intentional convention for app-specific design systems.
- Do not copy the current SIR / Workspace colors or style into another project unless its manager explicitly chooses that direction.

## Write `DESIGN.md`

Follow the [Google Labs DESIGN.md specification](https://github.com/google-labs-code/design.md/blob/main/docs/spec.md). Treat the format as alpha and verify the current spec when changing its requirements. Keep machine-readable YAML design tokens and human-readable guidance together in the root `DESIGN.md`; token values are normative and the prose explains their intended use.

Use the canonical `##` section order, omitting sections only when they do not apply and recording intentional omissions with reasons:

1. Overview
2. Colors
3. Typography
4. Layout
5. Elevation & Depth
6. Shapes
7. Components
8. Do's and Don'ts

Define a small, semantic token set that the project can implement: meaningful color roles, typography roles, relevant spacing and radius scales, and tokens for repeated components. Use token references such as `{colors.primary}` to keep component definitions aligned. Keep token names and values consistent with existing CSS variables and installed components. Avoid inventing values that conflict with the project's existing visual system without explaining and agreeing on the change.

Cover the project's audience, visual personality, contrast and focus expectations, responsive layout, depth, shapes, reusable component behavior, and concrete do/don't rules. If the project has no visual interface, create a concise guide that states that scope and records non-applicable token sections as intentional omissions rather than inventing a UI palette.

## Connect tokens to implementation

- For a SvelteKit console, make its shared `layout.css` reflect the root `DESIGN.md` tokens; keep CSS custom properties and semantic component styles aligned with the token values.
- Prefer suitable components already installed or imported (including shadcn-svelte when selected). Design reusable components to reduce repeated markup and CSS; specify custom visual patterns only where the available components do not fit.
- Do not redesign screens or change application CSS unless that implementation work is part of the request. `DESIGN.md` creation should leave a clear source of truth for later UI work.

## Validate and hand off

When the official CLI is available, validate the file with `npx @google/design.md lint DESIGN.md`. Resolve structural errors and broken token references; review contrast warnings and explain any intentional exceptions. If the CLI is unavailable, follow the linked specification and report that linting was not run rather than claiming validation.

Summarize the design choices, the `DESIGN.md` path, any stylesheet changes, validation performed, and unresolved decisions. Keep the guide specific to this project and do not copy long external documentation into it.
