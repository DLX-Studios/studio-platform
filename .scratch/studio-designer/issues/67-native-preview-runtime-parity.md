# 67 [R/D]: Render the authored canvas with shared Runtime controls

**Status:** open — High. **Blocked by:** 66.
**Source:** Designer decisions 04/05/10, ticket 39, audit F02, backlog C002/C003.

`FocusView::render_node` currently draws a label containing each node's kind and ID, followed by
nested labeled boxes. It does not render the authored Text value, native input control, declared
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
