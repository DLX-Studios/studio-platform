# 45 [D]: Workbench View and view-switching preservation

**What to build:** Persistent Workbench presentation keeping screens/hierarchy, Library entry point, inspector, diagnostics, interactions, agent activity, and history simultaneously visible. Switching between Focus and Workbench preserves project, screen, selection, profile, transform, tool, runs, history, and unsaved work; panel geometry is per-view. Command bar and keyboard shortcuts reach every capability from both views.

**Blocked by:** 39, 40, 43, 44

**Status:** ready-for-agent

- [ ] Switch views mid-edit mid-agent-run: nothing listed above is lost
- [ ] Every Workbench surface reachable from Focus View panels or command bar
- [ ] Per-view panel arrangement persists across restarts
