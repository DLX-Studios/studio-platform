# Feature Specification: `studio new` Project Scaffolding

**Feature Branch**: `011-studio-new`

**Created**: 2026-09-06

**Status**: Draft

**Input**: Starting a project today means copying `examples/starter` and
hand-deleting AssemblyScript remnants, then hand-converting to modern
Script (the exact pos-clothing-store journey). A scaffolding command
should produce a valid, building project in one step, from a gallery of
bases that are more than toys.

## Design decisions

- **Modern Script only, by definition.** `studio new` never generates
  AssemblyScript, `asconfig.json`, or `node_modules`. Assembly authors
  work from `examples/pos-desktop` as explicit reference; the
  scaffolder does not serve that path.
- **Templates are a gallery, not CLI strings.** Each template is a
  directory (`templates/<name>/` with `template.json` + project files)
  carrying its id, display name, description, and tags (`playground`,
  `plugin`, `layout`). The CLI expands them; the designer IDE reads the
  same gallery for its new-project flow. One source of truth.
- **Two template kinds.** `playground` templates are sandboxes for
  learning (commented, minimal). `plugin`/`layout` templates are
  working bases pre-wired for a vertical (cart state, detail panel,
  dialog flows) that authors extend rather than rewrite.

## Clarifications

### Session 2026-09-06

- Q: What does it generate? → A: A directory with `manifest.json`
  (fresh reverse-dns id from the project name, example publisher
  placeholder) and a minimal valid `app.studio` (header, one product
  card, one stateful counter button). No `assembly/`, no `asconfig.json`,
  no `node_modules`.
- Q: Templates? → A: One template: modern-script app. No `--assembly`
  variant (assembly authors start from `examples/pos-desktop`
  explicitly; a second template is future work if demanded).
- Q: What does "valid" mean? → A: Self-proving: the generated project
  passes `studio check` and `studio build` with zero diagnostics. The
  command runs check itself and reports failures instead of leaving a
  broken tree (it never writes partial output on invalid names).
- Q: Name rules? → A: Lowercase alphanumeric plus `-`; anything else is
  a usage error naming the rule. The id is `com.studio.<name with - to
  .>`. Existing non-empty directories are refused unless `--force`.
- Q: Where does it fit? → A: After 010 (it benefits from shape
  validation: the template is verified by the same gate users get).

## User Scenarios & Testing

### User Story 1 - Blank app in one command (Priority: P1)

An author runs `studio new pos-clothing-store`. The directory appears
with manifest + app; `studio check` and `studio build` pass immediately;
`studio dev` renders the starter screen.

Acceptance: CLI integration test creates the project in a temp dir,
asserts check/build exit zero, and asserts the bundle exists — plus a
usage-error test for bad names and non-empty directories.

### User Story 2 - Start from a working base (Priority: P1)

An author runs `studio new my-shop -t pos-retail` (or picks it in the
IDE gallery) and gets the transaction layout — product grid, detail
panel, cart state — wired and building. `studio new --list-templates`
shows every gallery entry with its one-line description.

### User Story 3 - Fetch a template remotely (Priority: P1)

An author runs `studio new my-shop -t owner/repo`, `-t
https://git.example.com/team/template.git`, or `-t
https://example.com/templates/pos-retail.studio` and gets the same
result as a gallery template: sources on disk, verified, building.
HTTPS only; GitHub shorthand resolves to codeload zips (no git binary
needed); direct links accept template zips or self-describing `.studio`
bundles (valid bundles that additionally carry `template.json` +
sources at their root).

Acceptance: fetch of each source kind in a sandbox expands byte-identical
trees to the local equivalent; non-HTTPS, missing `template.json`, and
failing self-check all fail closed with usage errors. Publishing a
template bundle (`studio pack --as-template`) is follow-up work, not
this ticket.

### User Story 4 - Self-describing playground (Priority: P2)

The generated `app.studio` is a commented miniature of the model: one
`$state` slot, one `$derived` slot, one zero-emit handler patching one
text node, each with a one-line comment pointing at the concept. It is
documentation that compiles.

## Functional Requirements

- FR-1: `studio new <name> [-t <template>] [--force] [--list-templates]`
  scaffolds `manifest.json` + `app.studio`; refuses bad names and
  occupied directories (without `--force`) with usage-family
  diagnostics.
- FR-2: Template resolution order: gallery name → GitHub shorthand →
  git URL → direct URL. Remote transport runs over system `curl`
  (HTTPS-only) and `git`, both ubiquitous dev tooling; absence fails
  closed with a usage error. Archive extraction uses the existing
  workspace `zip` crate. No new Rust dependencies.
- FR-3: Every expanded tree (local or remote) self-verifies with
  `studio check` before exit zero; remote trees additionally require a
  root `template.json` with matching id.
- FR-4: The blank template demonstrates state, derived, and a patch
  handler in under 40 lines with guiding comments.

## Gallery v1 (decided 2026-09-06)

- `blank` (playground, default): counter + derived + patch handler,
  heavily commented.
- `ecommerce` (layout): product grid, detail panel, cart state with
  derived totals — the pos-clothing-store shape, generalized.
- `social` (layout): Reddit-type feed — post cards with vote buttons
  and counts, comment threads, sort tabs, community sidebar.

Deferred: form flows, booking, inventory, component gallery.

## Non-Goals

- AssemblyScript template variant.
- Git init, editor config, README, or asset scaffolding.
- Interactive prompts.

## Success Criteria

- SC-1: `studio new demo && studio build demo` green from any cwd.
- SC-2: Full workspace gate green.
