# 67 [R/D]: Render the authored canvas with shared Runtime controls

**Status:** implementation in progress — shared renderer and TimePicker implemented; broad asset/profile and release acceptance remains. **Prerequisite 66:** complete.
**Source:** Designer decisions 04/05/10, ticket 39, audit F02, backlog C002/C003.

At the audit baseline, `FocusView::render_node` drew a label containing each node's kind and ID, followed by
nested labeled boxes. It did not render the authored Text value, native input control, declared
style/layout, or Runtime event behavior. A valid projection and session test cannot prove native
preview parity. Reuse the host-owned native renderer with editor selection/geometry overlays;
keep authoring commands and prototype input distinct while sharing component semantics.

## First failing regression

Add production-view GPUI UI integration coverage. Author Text and an interactive control, mount
Focus, edit the Text, and assert the rendered authored value changes through the normal command
path. Check native input/focus and prototype button events and compare against the Runtime view
from the same projection. A source-string assertion or count of projected nodes cannot satisfy it.

## Acceptance

- [ ] Focus renders real Runtime component values, styles/layout, input and event behavior.
- [ ] Selection overlays/hierarchy preserve stable node IDs without changing component semantics.
- [ ] Token, responsive profile, asset and interaction changes appear in native preview.
- [ ] Same projection renders consistently in Focus, Workbench preview and launched Runtime.
- [ ] TimePicker gains its semantic renderer/inspector/events and full catalog certification passes.
- [ ] Pointer, keyboard, accessibility, focus and reduced-motion cases use production views.

Expected scope: shared renderer seam around `studio-app/src/foundation.rs`,
`studio-designer/src/focus_view.rs`, `studio-components`, and native UI integration tests.
Read GPUI Kit coding/design guides before choosing the renderer seam or changing the visible UI.
Keep third-party renderer registration and arbitrary guest GPUI handles unavailable.

## Native implementation evidence — 2026-11-01

The first production-view test failed because Focus mounted no authored native Button. Focus
now consumes `studio-components::native_view::NativeRuntimeView`, shared with Runtime's thin
`FoundationGallery` wrapper. Selection forwards native layout unchanged and prototype actions
re-project ephemeral state. Script editor state is initialized with the view; asynchronous
project opens retain window/session identity. The native regression also exposed and fixed the
zero-width canvas layout and a large recursive debug stack frame.

Focused evidence: 56 Designer tests, including five native UI tests, and 58 component/protocol
tests pass. Native tests cover text selection and saved edits, input focus/typing, prototype
isolation, TimePicker keyboard and inspector edits, invalid-value rejection, and matching
control values/relative geometry between Focus and the shared Runtime renderer. Component
native tests cover independent time controls, disabled edits, host updates without feedback
loops, and a 32-container tree. The catalog certification gate now passes with zero fallback kinds.

This is not yet the signed Build/Run journey. Asset-byte import/loading remains C007, complete
Workbench interaction remains C008, and per-platform accessibility remains C019. Broader
profile/token/asset native cases remain to be recorded before closing every acceptance item above.
See [shared preview contract](../../../docs/development/DESIGNER_NATIVE_PREVIEW.md).

Clean-cache integration checkpoint: 1,302 workspace tests pass (two qualifications ignored),
Clippy passes with warnings denied, and all 39 Bun tests pass, including headless Wayland.
The cache rebuild used debug-symbol-free, nonincremental profiles and occupied about 12 GB.
