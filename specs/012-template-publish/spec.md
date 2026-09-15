# Feature Specification: Template Bundle Publishing

**Feature Branch**: `012-template-publish`

**Created**: 2026-09-06

**Status**: Draft

**Input**: `studio new` consumes remote templates, but nothing produces
them. Authors need a one-step publish from a working project to a
shareable `.studio` template bundle: the 011 follow-up.

## Clarifications

### Session 2026-09-06

- Q: New command or flag? → A: Flag: `studio build --as-template`.
  The 011 sketch said `studio pack`, but no pack command exists and the
  template bundle derives from a successful build. The normal app
  bundle is still written; the template bundle is an additional
  artifact `<name>.template.studio` beside it.
- Q: Why not embed sources in the app bundle? → A: The runtime
  archive (`inspect_archive`) rejects every non-`assets/` extra path,
  and relaxing it would drag unsigned files into the trust boundary.
  Template bundles are a separate format the runtime never accepts
  (it fails them closed as `LayoutInvalid`); `studio new` is their
  only reader. No signature: template bundles distribute sources, and
  the consumer rebuilds and re-signs.
- Q: Where does `template.json` come from? → A: The project root must
  contain one (same schema as the gallery). Absent file is a usage
  error naming it; no placeholder garbage is generated.
- Q: What goes inside? → A: `template.json`, `manifest.json`,
  `app.studio`, and declared `assets/` — never `*.wasm` or `build/`
  outputs. Byte-deterministic like app bundles (stored entries, fixed
  metadata, sorted order).
- Q: How is it verified? → A: Self-proving: after writing, the bundle
  is read back through the same checks `studio new` applies, and the
  round-trip test publishes a gallery template and scaffolds from it
  over the loopback server.

## User Scenarios & Testing

### User Story 1 - Publish the clothing store (Priority: P1)

An author adds `template.json` to a working project and runs `studio
build --as-template`. Beside the app bundle appears
`<name>.template.studio`. Posting that link lets anyone run `studio new
shop -t <link>` and get building sources.

Acceptance: round-trip integration test (publish gallery template →
serve over loopback → scaffold → check passes), byte-identical rebuilds,
and runtime rejection of template bundles as app bundles.

### User Story 2 - Missing manifest fails closed (Priority: P2)

Running `--as-template` in a project without `template.json` fails
with a usage diagnostic naming the file, and writes no artifact.

## Functional Requirements

- FR-1: `studio build [--as-template]`; template mode requires
  project-root `template.json` (usage error otherwise) and writes
  `<name>.template.studio` next to the app bundle.
- FR-2: `studio-package` owns `pack_template`/`inspect_template` with
  the archive conventions (stored, fixed metadata, sorted, size
  limits); template layout is exactly
  `template.json` + `manifest.json` + `app.studio` + `assets/*`.
- FR-3: The writer self-verifies by inspecting what it wrote; the
  runtime path is untouched and still rejects the format.
- FR-4: Determinism: two publishes of the same tree are byte-identical.

## Non-Goals

- Signatures on template bundles.
- Publishing assembly projects (entry must be a `.studio` source
  project; assembly trees fail with a usage diagnostic).
- Registry, versioning, or update checks for templates.

## Success Criteria

- SC-1: Round-trip test green (publish → serve → scaffold → check).
- SC-2: Full workspace gate green.
