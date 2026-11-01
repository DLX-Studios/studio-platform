# Shared native preview

The Runtime shell and Designer render through `studio-components::native_view::NativeRuntimeView`.
`NativeSurface` supplies an immutable tree, admitted assets, semantic input dispatch and host-action
polling. Runtime implements it with the admitted `PluginSurface`; Designer supplies the current
session projection. Neither guests nor project files can install native renderers or receive GPUI
entities. Runtime retains its own package admission and reload lifecycle.

Focus mounts actual controls rather than labeled node boxes. A geometry-transparent selection
element forwards the child's layout and paints an outline without adding a layout container.
Authoring pointer capture selects the stable source ID. Prototype mode removes that capture and
routes native actions to the isolated `PrototypeSession`; navigation, visibility and property
effects re-project that ephemeral source using the same profile and Library mapping. Exiting the
prototype restores the saved source and clears native input buffers from the prototype session.
Normal host patches preserve unrelated native state. Retired nodes release retained widget state.

The selected device profile now controls both responsive variant resolution and the native
canvas content viewport, including safe-area deductions. Larger profiles scroll in both axes
inside the canvas pane without resizing the inspector. Profile changes do not create revisions.
Native tests verify phone-through-4K control widths, responsive text/gap overrides, and a shared
spacing-token edit reaching measured native layout.

The toolbar sits above the canvas/inspector row, and the inspector scrolls independently. The
native Script editor is created with the view rather than on its first render. Asynchronous
project opens require the live window and retain the session/generation checks from the ownership
contract.

TimePicker is a leaf control. Its optional `value` is a canonical local 24-hour `HH:MM` or
`HH:MM:SS` string, defaulting to `00:00`; seconds determine the editing precision. `label`,
`enabled`, `on_changed` and the common protocol properties are accepted. Invalid times, zones,
locale-dependent text and arbitrary children fail protocol validation. Each stable node retains
its own segmented native state. Keyboard edits emit `changed` with `{ "value": "HH:MM[:SS]" }`;
host patches do not synthesize user events. Designer's time inspector stages text and commits
only a valid value through the shared command path.

The native tests render production views in GPUI headless windows, dispatch pointer/keyboard
input and inspect native accessibility values, geometry and application outcomes. They cover
saved text edits, prototype isolation, time editing and host patches, disabled behavior,
independent controls and a 32-container tree on the default test stack. Focus/Runtime renderer
comparison is distinct from building and launching a signed application; C005/C006/C015 own that
acceptance. Library import/byte loading, complete Workbench panels and target-OS accessibility
acceptance remain in C007/C008/C019. The catalog readiness table has zero unsupported kinds;
that table does not claim every OS-native journey has been executed.
