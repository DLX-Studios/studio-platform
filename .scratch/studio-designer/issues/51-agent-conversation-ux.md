# 51 [D]: Agent composer, floating conversation, and Agent References

**What to build:** Clean welcome surface with spacious composer — company mark left, searchable model/provider selector right, scoped context attachments, Import, send; no terminal-switch or full-access controls. Selected model recorded per run. First message transitions thread into a movable/resizable/collapsible floating window that survives view switches and supports float/left/right docking per the application-shell prototype. Inline typed Agent Reference chips render with type icons and current labels; activation navigates/selects only.

**Blocked by:** 45, 50

**Status:** ready-for-agent

- [ ] Prototype behaviors reproduced: welcome→first-message transition→floating persistence across view switches
- [ ] Docking layouts float/left/right all function with keyboard support
- [ ] Reference activation opens target; renamed target shows current label retaining identity
- [ ] Deleted/denied targets remain visible chips with explanation
- [ ] Model/run provenance recorded on messages, batches, and undo groups
