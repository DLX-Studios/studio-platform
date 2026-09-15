# 45 [D]: Workbench View and view-switching preservation

**What to build:** Persistent Workbench presentation keeping screens/hierarchy, Library entry point, inspector, diagnostics, interactions, agent activity, and history simultaneously visible. Switching between Focus and Workbench preserves project, screen, selection, profile, transform, tool, runs, history, and unsaved work; panel geometry is per-view. Command bar and keyboard shortcuts reach every capability from both views.

**Blocked by:** 39, 40, 43, 44

**Status:** done

- [x] Switch views mid-edit mid-agent-run: nothing listed above is lost — shared session context and repeated Focus/Workbench reads are proven at `crates/studio-design/tests/workspace_view.rs:51-93`; native switching delegates through one session at `crates/studio-designer/src/focus_view.rs:1328-1332` and `2243-2266`.
- [x] Every Workbench surface reachable from Focus View panels or command bar — all eight `PanelId` entries have command/shortcut descriptors at `crates/studio-design/src/workspace.rs:443-490`, and Workbench renders each surface at `crates/studio-designer/src/focus_view.rs:2267-2335`.
- [x] Per-view panel arrangement persists across restarts — schema-versioned load/save is wired at `crates/studio-designer/src/focus_view.rs:1281-1326`; independent geometry and reopen persistence are covered at `crates/studio-design/tests/workspace_view.rs:96-140`.
