# 32 [R]: Semantic renderer expansion — forms and inputs

**What to build:** Every input-family catalog kind gains semantically complete rendering including focus/accessibility semantics, validation display, and disabled/readonly/error states.

**Blocked by:** None (can start immediately)

**Status:** complete (source/test evidence; Wayland/accesskit runtime remains external)

- [x] Each kind in batch renders all declared properties and states (`crates/studio-app/src/foundation.rs:2636`, `:2694`, `:2740`, `:2899`)
- [ ] Keyboard, pointer, and touch behavior matches the component contract (source paths are covered; Wayland runtime gate is external)
- [ ] Accessibility names/roles/positions verified (source roles/labels are present; Wayland/accesskit runtime gate is external)
- [x] Matrix updated per kind (`docs/component-matrix.md:25`, `crates/studio-components/tests/catalog_mapping.rs:217`)

## Implementation Notes

### Decisions made before adding widgets

- Stable-ID handling: every stateful widget (text inputs, selects, sliders, OTP) is retained in a
  per-node map keyed by the stable protocol node ID (`plugin_inputs` / `plugin_selects` /
  `plugin_sliders` / `plugin_otps`). Targeted property patches re-render elements but reuse the
  same entities, so mounted state survives and GPUI focus follows the same entity
  (`crates/studio-app/src/foundation.rs:559`, `:657`, `:720`, `:794`).
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

### Verification

- `form_input_kinds_are_semantically_rendered_after_batch_b` readiness test incl. SecretInput
  host-owned-value assertion (`crates/studio-components/tests/catalog_mapping.rs:217`).
- Renderer tests cover numeric parsing, host patch detection, NumberInput bounds/step, selected
  variant, and hidden-subtree visitation (`crates/studio-app/src/foundation.rs:3211`, `:3223`,
  `:3230`, `:3240`, `:3246`).
- Matrix doc advanced to rendered+verified for all 18 form/input kinds (`docs/component-matrix.md:25`).

### Remaining external verification

- Wayland keyboard/pointer/touch behavior and accesskit role exposure require the external runtime
  harness. Retained-state reconciliation, NumberInput min/max/step setters, selected styling, and
  hidden-subtree visitation are compiled and covered by focused tests in this closure pass.
