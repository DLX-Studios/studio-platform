# 54 [D]: Product welcome, account chooser, and Local Identity authentication

**What to build:** Full-screen product welcome (dismissal remembered, revisitable from Help); startup identity discovery; account chooser distinguishing identity kinds and session states; Local Identity create with display name/email/password/optional avatar/remembered-session preference; fully offline sign-in and locked-session unlock; revocable remembered sessions; multiple identities per device; salted verifiers never raw passwords.

**Blocked by:** 14, 37

**Status:** partial — native local identity forms exist; project ownership and full native acceptance remain open

- [ ] Fresh-directory first launch walks welcome→chooser→create→project entry
- [ ] Reopen honors dismissal memory
- [ ] Wrong password locks; unlock requires password; remembered session resumes and is revocable from another session's view
- [ ] Multiple local identities keep projects isolated pre-authentication
- [ ] Entire journey functions with networking disabled

## Implementation notes

Product welcome GPUI surface now follows application-shell Monolith (variant A): native title bar, split copy/board, canvas-mono tokens, `Get started →` dismisses to the identity chooser, `Open local identity` dismisses then routes to sign-in/unlock or create. That earlier stub-shell assessment is superseded: native chooser/create/sign-in/unlock inputs are wired in `bootstrap.rs`; ownership and complete native acceptance remain open.

## Completion audit — 2026-11-01

Current source/evidence assessment: F01/F09 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
