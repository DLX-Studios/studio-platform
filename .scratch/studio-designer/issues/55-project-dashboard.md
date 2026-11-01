# 55 [D]: Project Dashboard — Grid, Index, Activity

**What to build:** Post-authentication home presenting one persisted catalog query through three user-switchable modes: Grid cards, Index list, Activity feed combining safe recent activity. Search over names/admitted metadata; filters local/cloud/syncing/conflicted/archived; sorting by activity/name/created/updated/state; project lifecycle rename/duplicate/archive/restore/informed delete; empty state emphasizing Create/Import/Templates; backward navigation editor→chooser→identity matching the prototype.

**Blocked by:** 54, 37

**Status:** partial — persisted dashboard/basic native project list exists; ownership, full modes/lifecycle and keyboard acceptance remain open

- [ ] Mode switching preserves query state, filters, sort, and selection
- [ ] Delete explains local/cloud/asset/backup/unsynced consequences before confirming
- [ ] Activity summaries never disclose protected content before authentication
- [ ] Resume-last-project failure returns to dashboard with recovery diagnostic, not a trap
- [ ] Keyboard navigation complete across all three modes

## Completion audit — 2026-11-01

Current source/evidence assessment: F01/F09 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
