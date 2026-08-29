# 33 [R]: Semantic renderer expansion — feedback, navigation, data display

**What to build:** Remaining families — feedback, overlays/dialogs, navigation, and data-display kinds — gain semantically complete native rendering with matrix advancement.

**Blocked by:** None (can start immediately)

**Status:** complete (source/test evidence; overlay/accesskit runtime remains external)

- [x] Each kind in batch renders all declared properties correctly (`crates/studio-app/src/foundation.rs:1757`, `:1941`, `:2150`, `:2240`)
- [ ] Overlay kinds stack/focus/dismiss per contract (host gating is source-backed; Tab/backdrop/accesskit runtime gate is external)
- [ ] Data-display kinds handle empty, loading, error, and populated states (empty/populated are rendered; loading/error are not representable in the closed schema)
- [x] Matrix updated per kind (`docs/component-matrix.md:47`, `crates/studio-components/tests/catalog_mapping.rs:255`)

## Implementation Notes

### Overlay contract (host-owned)

- Gating/stacking/dismissal live behind `overlay_gate` / `overlay_root`: visible overlays receive
  a monotonically increasing depth (per render pass) used for unique IDs; this gpui build exposes
  no z-index, so stacking follows tree paint order.
- Dismissal is host-owned: Escape (all gated overlays) or click (Toast/Notification) hides the
  overlay locally via `dismissed_overlays`; the entry resets when the protocol reports
  `open=false` (or a missing toast/notification message), so reopening works without remounts.
- Reduced motion: entrance fades are skipped entirely under reduced motion; otherwise a single
  150 ms opacity ease runs.
- Focus: each open overlay gets a retained `FocusHandle` keyed by node ID (`overlay_focus`);
  full Tab cycling inside overlays remains an external runtime check.

### Renderer work

- Dialog: rebuilt on overlay machinery with `Role::Dialog`, title, children, Escape dismissal,
  reduced-motion-aware fade.
- AlertDialog: modal with title + message + children. Sheet/BottomSheet/Drawer: edge-docked
  panels gated by `open`. ContextMenu: centered menu surface with message header + children
  (the closed schema declares no placement property — noted, not invented).
- CommandPalette: `open`-gated palette rendering placeholder + declared `commands`, empty state
  when none. Popover: native popover kept, now honoring `open` gating, opacity/aria, and real
  children content instead of hardcoded text.
- Toast/Notification: message-driven floating surfaces (message absent = closed) with host-owned
  click dismissal. Banner: inline alert strip.
- Tooltip: children wrapper with a native hover tooltip built from the declared message.
- Navigation: AppBar/Sidebar/Scaffold/Tabs/Breadcrumb/StatusBar/NavigationBar/NavigationRail gain
  roles, opacity, aria labels, orientation handling (rail vertical), Tabs renders declared item
  labels as tab headers (selection stays per-child via `selected`; no invented selection state).
  Stepper/Pagination render step/page/pages from declared props.
- Data display: ListTile (label + trailing children), SearchableList/VirtualList (scrollable
  lists), DataTable (declared columns header + rows), Tree (indented hierarchy),
  DescriptionList (two-column grid). All show a shared empty-state placeholder when their
  declared collections and children are absent.
- ProgressIndicator/ProgressCircle/Spinner were already semantically rendered (value/label);
  readiness now records it.

### State semantics note (closed schema)

Loading/error states are not representable under the closed protocol schema for data-display
kinds (no such properties exist), so only empty and populated states are implemented; nothing was
invented beyond the declared schemas.

### Verification

- `overlay_navigation_and_data_display_kinds_are_rendered_after_batch_c` readiness test covering
  all certified advanced kinds (`crates/studio-components/tests/catalog_mapping.rs:255`).
- `reads_declared_string_list_properties_for_data_display` and native surface tests pass
  (`crates/studio-app/src/foundation.rs:3267`).
- Matrix doc advances all formerly deferred kinds; TimePicker remains explicitly no/no with its
  rationale (`docs/component-matrix.md:77`).

### Remaining external verification

- Wayland focus trapping, backdrop dismissal, tooltip interaction, and accesskit role coverage
  remain external runtime gates. ContextMenu remains centered because its closed schema declares
  no placement property.
