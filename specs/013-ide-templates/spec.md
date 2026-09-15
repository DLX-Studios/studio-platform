# Feature Specification: Designer IDE Template Gallery

**Feature Branch**: `013-ide-templates`

**Created**: 2026-09-06

**Status**: Draft

**Input**: The template gallery (`templates/`) is CLI-only. The designer
IDE (VS Code extension) has no new-project flow, even though the gallery
was designed as its shared source. Authors in the IDE should pick a
template and name a project without touching the terminal first.

## Clarifications

### Session 2026-09-06

- Q: How does the IDE read the gallery? → A: Through the CLI it
  already shells to: `studio new --list-templates --format json`
  (new machine-readable mode) for the QuickPick, then a terminal
  `studio new` for the scaffold — same thin-client shape as every
  other command. No compiler logic enters the extension.
- Q: Why a JSON mode instead of parsing display lines? → A: Display
  text is human-owned and will drift; the contract is the JSON array
  `[{id, kind, description}]`, covered by a CLI test.
- Q: Where does the project go? → A: Folder picker, defaulting to the
  workspace root; the scaffold runs in a `studio` terminal there. An
  "Open Folder" button after success hands off to the new project
  (user-initiated, never an automatic window restart).
- Q: Name validation? → A: Instant client-side check mirroring the CLI
  slug rule (shared pure function, unit-tested); the CLI remains
  authoritative and its errors surface in the terminal.
- Q: How is the extension logic tested? → A: Pure helpers
  (`parseTemplateList`, `buildNewCommand`, `isValidProjectName`) live
  in `src/templates.ts` with zero `vscode` imports, covered by
  `bun test`; `extension.ts` stays wiring-only.

## User Scenarios & Testing

### User Story 1 - New project from the IDE (Priority: P1)

An author runs "Studio: New Project…", picks `social` from the gallery
list, names it, picks a folder. The terminal scaffolds; the success
button opens the new folder. At no point is a terminal command typed.

Acceptance: bun tests for the pure helpers (parse/list/command/valid
cases); `tsc` clean; manual pass per the extension quickstart (command
appears, lists gallery, scaffolds, opens folder).

### User Story 2 - Missing CLI degrades honestly (Priority: P2)

With no `studio` binary, the command shows the same degraded warning
as other commands instead of hanging or throwing.

## Functional Requirements

- FR-1: `studio new --list-templates --format json` emits the gallery
  array; human text format is unchanged and default.
- FR-2: `studio.newProject` command + activation event + palette
  entry; QuickPick (name + description + kind), name input with
  instant validation, folder picker, terminal scaffold, open-folder
  button on success hint.
- FR-3: No `vscode` imports in `templates.ts`; bun-tested pure logic.

## Non-Goals

- Remote-template browsing in the IDE (paste a URL as the template;
  the CLI resolves it).
- Scaffolding outside terminals (progress UI, cancellation).
- Extension integration tests under a VS Code harness.

## Success Criteria

- SC-1: `bun test` + `tsc` green in `editors/vscode`.
- SC-2: Full workspace gate green.
