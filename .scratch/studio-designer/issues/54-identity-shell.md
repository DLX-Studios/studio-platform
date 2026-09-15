# 54 [D]: Product welcome, account chooser, and Local Identity authentication

**What to build:** Full-screen product welcome (dismissal remembered, revisitable from Help); startup identity discovery; account chooser distinguishing identity kinds and session states; Local Identity create with display name/email/password/optional avatar/remembered-session preference; fully offline sign-in and locked-session unlock; revocable remembered sessions; multiple identities per device; salted verifiers never raw passwords.

**Blocked by:** 14, 37

**Status:** ready-for-agent

- [ ] Fresh-directory first launch walks welcome→chooser→create→project entry
- [ ] Reopen honors dismissal memory
- [ ] Wrong password locks; unlock requires password; remembered session resumes and is revocable from another session's view
- [ ] Multiple local identities keep projects isolated pre-authentication
- [ ] Entire journey functions with networking disabled

## Implementation notes

Product welcome GPUI surface now follows application-shell Monolith (variant A): native title bar, split copy/board, canvas-mono tokens, `Get started →` dismisses to the identity chooser, `Open local identity` dismisses then routes to sign-in/unlock or create. Chooser, create, sign-in, and unlock screens are still the stub shell and remain in this ticket.
