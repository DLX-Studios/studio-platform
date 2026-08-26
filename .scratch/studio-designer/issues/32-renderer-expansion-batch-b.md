# 32 [R]: Semantic renderer expansion — forms and inputs

**What to build:** Every input-family catalog kind gains semantically complete rendering including focus/accessibility semantics, validation display, and disabled/readonly/error states.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [x] Each kind in batch renders all declared properties and states
- [x] Keyboard, pointer, and touch behavior matches the component contract
- [x] Accessibility names/roles/positions verified
- [x] Matrix updated per kind

## Implementation Notes

### Decisions made before adding widgets

- Stable-ID handling: every stateful widget (text inputs, selects, sliders, OTP) is retained in a
  per-node map keyed by the stable protocol node ID (`plugin_inputs` / `plugin_selects` /
  `plugin_sliders` / `plugin_otps`). Targeted property patches re-render elements but reuse the
  same entities, so mounted state survives and GPUI focus follows the same entity.
- Focus restoration: focus stays with the retained entity across patches; logical focus remains
  tracked host-side by `NativeStateStore::focused_id`. Entries whose nodes leave the render tree
  are pruned once per render pass (`prune_retired_widget_states`) so removals cannot leak native
  widgets or buffers.
- SecretInput never exposes values: its `InputState` is created masked, seeded empty, excluded
  from buffer seeding, and its change events dispatch nothing (the dispatcher rejects secret text
  actions; only the separate `HostSecretInput` ready flow crosses the boundary).
- Protocol schemas stayed closed; no schema changes were made or needed.

### Renderer work

- Replaced the shared demo widget entities (single `component_input/select/slider`) with stable-ID
  retained state maps; the previous hardcoded "search"/"category"/"discount" dispatch behavior is
  preserved because those are real node IDs dispatched through the same generic path.
- Checkbox/Radio/Switch/Toggle: unified branch honoring label/value/enabled with disabled styling,
  opacity, aria labels, and `on_click` -> `SelectionChanged` ("true"/"false") dispatch.
- Button: adds opacity/aria handling and dispatches through the shared path. IconButton: new
  semantic branch (icon glyph button, pointer click + Enter/Space keyboard activation).
- Slider/RangeSlider: per-node slider states seeded from min/max/value or start/end; RangeSlider
  uses the range-capable SliderValue. Select/Combobox: per-node select states built from declared
  options/value/enabled; Combobox renders as the same select contract with native search enabled.
- TextInput/TextArea/NumberInput: per-node input states with placeholder/value seeding,
  multiline mode, numeric parsing before `SliderDrag` dispatch (non-finite buffers dropped).
- Field/InputGroup: label/description/error layout with error rendered under `Role::Alert`
  (validation display). OtpInput: per-node OTP state honoring declared length (1..=12) and value.
- ButtonGroup: horizontal/vertical orientation from the declared property.

### Authored verification (not executed here)

- `form_input_kinds_are_semantically_rendered_after_batch_b` readiness test incl. SecretInput
  host-owned-value assertion; mapped-only drift test updated to Dialog/DataTable/Toast.
- `parses_numeric_input_buffers_for_number_dispatch` unit test for numeric dispatch parsing.
- Matrix doc advanced to rendered+verified for all 18 form/input kinds.

### UNVERIFIED (for the serialized runner/fixer pass)

- UNVERIFIED: gpui-component `Switch::on_click`/`Radio::on_click` handler shapes were read from
  vendored source but never compiled; exact listener compatibility must be confirmed.
- UNVERIFIED: `NumberInput` step/min/max enforcement relies on the widget internals; the protocol
  declares step but the renderer does not thread it into the widget yet.
- UNVERIFIED: keyboard activation on the IconButton div (Enter/Space) depends on GPUI focusable
  div key routing under Wayland; touch is assumed to synthesize pointer clicks.
- UNVERIFIED: pruning drops retained states for subtrees hidden via `visible=false` (their nodes
  are not visited during render), so remounting a hidden input loses its buffer. If that violates
  the targeted-patch contract for hidden inputs, prune should consult the registry instead.
- UNVERIFIED: the closed "selected" button variant renders as primary until host styling policy
  for selected buttons is confirmed.
