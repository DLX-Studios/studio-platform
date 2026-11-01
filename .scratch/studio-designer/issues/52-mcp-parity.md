# 52 [D]: MCP client parity

**What to build:** MCP clients connect to the identical scoped query and command interfaces as agents — same receipts, conflicts, scopes, and audit attribution. No privileged mutation path exists for MCP.

**Blocked by:** 50

**Status:** partial — scoped domain McpClient tested; native McpSurface is a no-op and must be wired

- [ ] Scripted MCP client performs equivalent edits with equivalent receipts
- [ ] Scope denials match agent behavior exactly
- [ ] Audit attributes actor kind MCP distinctly

## Completion audit — 2026-11-01

Current source/evidence assessment: F06 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
