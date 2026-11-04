---
name: studio-console-init
description: "Plan, create, or update the default project Console web app, including app pages and protected admin planning. Use during whole-project initialization or when maintaining an existing Console."
---

# Studio Console Init

Own the project Console web app: its public project/app pages, persistence, sign-in, and default admin planning experience. Run this skill as part of whole-project initialization or for a direct Console request. Console inclusion is already the default; do not ask whether the user wants or needs it. Honor an explicit exclusion and reuse an existing Console. Preserve the repository-shape decision.

Before asking the project manager to choose Console scope, data, or admin-planning behavior, read and follow [visual interaction for Studio planning](references/visual-interaction.md). Use available structured controls, a route map, or a compact comparison visual when those make the decisions easier to review.

## Inspect and confirm scope

- Read root `AGENTS.md`, `STUDIO.md`, `DESIGN.md`, workspace manifests, and existing console routes/styles. Use an existing console if present; update it rather than scaffolding a duplicate.
- Derive Console placement from the agreed shape: `/apps/console` for a multi-app/platform workspace, or a separate peer `project-console` repository for an umbrella. In a single-application project, use the repository root when the Console is the primary web app; otherwise use a companion `console/` folder and preserve the main product's source layout. Honor an existing or explicitly chosen location. Resolve repository-shape questions through `studio-project-init` when necessary; do not reopen whether to include the Console.
- Ensure the Console app root has an `AGENTS.md`. For `/apps/console`, create or update `apps/console/AGENTS.md` with the selected SvelteKit/runtime/database/auth stack, target and bind assumptions, code map, actual commands, applicable project decisions/specs, and relevant available skills (including when to use them). Link to inherited workspace guidance and avoid copying it wholesale. When the Console is the root single app, maintain the repository-root `AGENTS.md` instead of creating a duplicate nested file.
- Ensure the Console app root also has an `APP.md` using the canonical schema in root `STUDIO.md`, including the selected Web app/site type, implementation stack, browser/server targets with support evidence, purpose, and ownership boundaries. The project's Console may index this brief, but changes to these app identity fields update `APP.md` in the same change; keep the DB projection synchronized and flag unresolved sync instead of silently splitting sources of truth. For a root single-app Console, place `APP.md` at the repository root.
- Reuse the purpose, structure, user, and appearance decisions already collected. Ask only about missing choices that materially change the console, especially whether it catalogs/links to apps, embeds them, launches them, or reverse-proxies their interfaces.
- Check for the project-root `DESIGN.md`. Use it as the shared visual source by default. If it is missing, call `studio-design` with the agreed purpose and appearance context before implementing the console; do not duplicate appearance questions.

## Console web foundation

- Use SvelteKit with `@sveltejs/adapter-node`.
- The default runtime is Bun. Honor an explicit project-manager choice; otherwise prefer installed Bun, then pnpm, then Node with npm. Keep scripts, lockfiles, CI, and setup instructions consistent.
- Serve on port **8010** by default and make host binding configurable. Bind to a host network interface by default (typically `0.0.0.0`), not loopback only. Do not expose `/admin` until server-side authentication and authorization are active.
- In a multi-app workspace, the app path is `/apps/console` unless the project manager chose another name. For an umbrella, keep the Console in a separate peer repository as agreed.

## Persistence and database choice

- Propose one SQLite database for all persistent project entities by default, not a separate database per feature. Include users, projects/apps, catalog values, tickets, issues, screens, features, and decisions as applicable.
- With Bun as the runtime, use the built-in `bun:sqlite` driver. Version schema migrations, keep runtime database files out of Git, and document the persistent path and backup/restore procedure. See [Bun SQLite documentation](https://bun.sh/docs/runtime/sqlite).
- Ask whether the project manager wants a different database. If they choose one, invoke `$find-skills` to search for and read a relevant database setup skill when available, then verify the integration with current official Bun and provider documentation. If no suitable skill is available, continue with those primary docs.
- `bun:sqlite` requires the Bun runtime, not merely Bun as a package manager. If Node is the selected runtime, explain the constraint and ask whether to use Bun or a compatible driver; do not silently change the selected runtime.

## Email/password access and recovery

- Use email-and-password sign-in for Console administrators by default. With Bun, prefer asynchronous `Bun.password.hash` and `Bun.password.verify` (Argon2id by default with an automatically generated salt). For another runtime, use a vetted Argon2id or scrypt library. Never store or log plaintext passwords or reversible password values. Follow the [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html) and [Bun password hashing documentation](https://bun.sh/docs/runtime/hashing).
- Enforce admin roles on the server; hiding navigation or relying on `/admin` route names is not access control. Ask how the first admin is provisioned and whether accounts are invitation-only or may self-register.
- Include a root `scripts/reset-password.ts` utility for operator-assisted recovery. It generates a strong temporary password for a specified account, requires explicit confirmation, stores only its hash, forces a password change at next sign-in, and displays the generated value once to the operator. Never expose it as an unauthenticated web endpoint or log the value. Document how to run it with the chosen runtime/database; default Bun invocation: `bun run scripts/reset-password.ts`.

## Admin planning experience

Include the admin planning area by default unless the project manager explicitly excludes it. Keep `/admin` admin-only and use these routes:

- `/admin/planning/tickets`
- `/admin/planning/issues`
- `/admin/planning/screens`
- `/admin/planning/features`
- `/admin/planning/decisions`

Clarify how tickets differ from issues, how screens and features relate, whether decisions link to those records, and which system owns the data. Roles, persistence details, and external integrations remain project decisions.

## Design and components

- Map the shared `layout.css` to the root `DESIGN.md` tokens. In a multi-app workspace, default to `apps/console/src/lib/styles/layout.css`; for a root single app use the corresponding `src/lib/styles/layout.css` location.
- Prefer suitable shadcn-svelte components already imported or installed, and use the shadcn-svelte skill if available. Add custom components only when the available component set has no suitable fit; make repeated patterns reusable to avoid duplicated markup and CSS.
- Keep implementation aligned with the project design tokens and existing component architecture. Do not apply WorkspaceDev's current palette to another project unless its manager chooses it.

## Plan, implement, and hand off

For a substantial new Console, present a short plan covering pages, app placement, data entities, database choice, account provisioning, network exposure, design source, and open decisions before broad implementation. Implement the agreed scope, update its README and `STUDIO.md` details, and report commands actually run. Do not claim tests, deployment, or network access were verified unless they were. Use `studio-commit` for a separate checkpoint when requested.
