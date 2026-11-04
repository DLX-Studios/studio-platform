---
name: studio-init
description: "Route project setup work to the focused Studio skill for whole-project planning, nested apps, Console, or design. Use when the requested initialization scope is unclear or spans these workflows."
---

# Studio Init

Act as the entrypoint and router for Studio project setup. Keep project-specific architecture and implementation details in the focused skills listed below. Read local `AGENTS.md` and `STUDIO.md` when present, classify the user's request, and pass the relevant project context and decisions to the selected skill.

## Route by request and project state

| User's goal | Route to |
|---|---|
| Scaffold a Studio Platform app with the CLI in an already chosen folder/layout | `studio-scaffold`, when available |
| Write Studio Script app features after the project exists | `studio-code`, when available |
| Start, plan, or broadly reorganize an entire project/repository | `studio-project-init` |
| Add one runnable app to an established `/apps/` workspace | `studio-app-init` |
| Create or update the project Console web app | `studio-console-init` |
| Create or revise the project's visual system or `DESIGN.md` | `studio-design` |
| Checkpoint changes, update the changelog, commit/push, or compact | `studio-commit` |

Use the user's answers to choose the route. When a request spans stages, start with `studio-project-init` for unresolved whole-project structure, then let it route agreed nested-app, Console, and design work to their skills. If only one stage is requested and its prerequisites are already settled, route directly without repeating discovery.

`studio-scaffold` and `studio-code` are focused Studio Platform workflows;
they do not require whole-project planning for an otherwise clear app request.

## Shared routing rules

- Before asking a routing question or presenting a combined set of setup decisions, read and follow [visual interaction for Studio planning](references/visual-interaction.md). Use the current host's structured choice controls or visual presentation tools when available.
- If a project has a root `STUDIO.md`, pass it as authoritative project context. If it is missing during whole-project initialization, route to `studio-project-init` to create it.
- Whole-project initialization includes the Console web app by default. Include it in the plan and route its creation or update to `studio-console-init`; mentioning it without a handoff is incomplete. Do not ask whether the user wants or needs a Console. Honor an explicit exclusion, and reuse an existing Console. A focused app-only or code-only request keeps its requested scope.
- `studio-project-init` and `studio-app-init` check for a project-root `DESIGN.md`. If missing, they call `studio-design` and pass along any visual direction already collected. If present, use it by default and do not create a duplicate.
- Pass context, user choices, research links, constraints, and unresolved questions between skills. Do not make the user answer the same question twice.
- If the requested scope is ambiguous between a whole-project restructure and adding one nested app, ask one concise question before routing. Otherwise proceed with the best-matching skill.
- Do not perform the full workflows of routed skills in this entrypoint. Resume only to coordinate their handoff and summarize the combined result.
