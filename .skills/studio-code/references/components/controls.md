# Inputs and actions

Static examples demonstrate valid component shapes. Wire named handlers using the shared reactive examples in contracts.md; there is no bind:value or DOM event object support.

See [shared contracts](contracts.md) for types, events, state, and validation.

## Button

Activate a named app handler.

**Props:** `label`: nonempty string; `enabled`: boolean; `variant`: `primary`, `secondary`, `selected`; `width`: `intrinsic` or `full`; protocol event prop `on_pressed`.

**Children:** none. **Events/state:** onclick={handler} receives the pressed trigger. See the complete reactive example in contracts.md.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Button id="example-button" label="Save" variant="primary" enabled={true} />
```

## IconButton

Activate an icon-only action.

**Props:** `icon`: nonempty icon name; `enabled`: boolean; protocol event prop `on_pressed`. Give icon-only actions an accessibility label.

**Children:** none. **Events/state:** onclick={handler} receives pressed.

**Current support:** Native icon action is wired. The protocol accepts accessibility metadata, but the installed source checker rejects `accessibility_label` on this tag. This example omits it to remain checkable; use a labeled Button when an accessible action name is required until that source-checker gap is resolved.

```svelte
<IconButton id="example-icon-button" icon="search" />
```

## Checkbox

Choose an independent boolean option.

**Props:** `label`: nonempty string; `value`, `enabled`: booleans; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler} receives changed; the native value is a string "true"/"false". See event binding limits in contracts.md.

**Current support:** Boolean value is controlled by guest props.

```svelte
<Checkbox id="example-checkbox" label="Enabled" value={true} enabled={true} />
```

## Radio

Choose one option in an app-managed group.

**Props:** `label`: nonempty string; `value`, `enabled`: booleans; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler} receives changed; the native value is a string "true"/"false". See event binding limits in contracts.md.

**Current support:** Radio grouping is app-owned; no automatic RadioGroup tag is declared.

```svelte
<Radio id="example-radio" label="Enabled" value={true} enabled={true} />
```

## Switch

Toggle a persistent setting.

**Props:** `label`: nonempty string; `value`, `enabled`: booleans; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler} receives changed; the native value is a string "true"/"false". See event binding limits in contracts.md.

**Current support:** Boolean value is controlled by guest props.

```svelte
<Switch id="example-switch" label="Enabled" value={true} enabled={true} />
```

## Toggle

Represent a boolean action state.

**Props:** `label`: nonempty string; `value`, `enabled`: booleans; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler} receives changed; the native value is a string "true"/"false". See event binding limits in contracts.md.

**Current support:** Boolean value is controlled by guest props; Toggle currently uses the native switch presentation.

```svelte
<Toggle id="example-toggle" label="Enabled" value={true} enabled={true} />
```

## ButtonGroup

Lay out related action buttons.

**Props:** `orientation`: `horizontal` or `vertical`.

**Children:** any. **Events/state:** Attach onclick handlers to the individual Buttons; the group is a layout wrapper.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<ButtonGroup id="example-button-group" orientation="horizontal">
  <Button id="group-one" label="Back" />
  <Button id="group-two" label="Next" />
</ButtonGroup>
```

## Slider

Choose a scalar in a bounded interval.

**Props:** `label`: nonempty string; `min`, `max`, `value`: finite numbers; `enabled`: boolean; protocol event prop `on_changed`. When both bounds are present, require min < max and min ≤ value ≤ max.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries a finite numeric value.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Slider id="example-slider" label="Volume" min={0} max={100} value={50} />
```

## RangeSlider

Choose a two-ended numeric interval.

**Props:** `label`: nonempty string; `min`, `max`, `start`, `end`: finite numbers; `enabled`: boolean; protocol event prop `on_changed`. Keep start/end ordered within the bounds.

**Children:** none. **Events/state:** onchange={handler}; current dispatch emits one numeric value rather than a start/end record.

**Current support:** Two native thumb values render, but end-to-end range event payload is limited; verify before relying on both endpoints.

```svelte
<RangeSlider id="example-range-slider" label="Price" min={0} max={100} start={20} end={80} />
```

## Select

Choose one of a bounded set of strings.

**Props:** `label`, `value`: nonempty strings; `options`: nonempty unique string list; `enabled`: boolean; protocol event prop `on_changed`. Select value must occur in options.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries a selected string.

**Current support:** Native selector is wired; keep value inside options.

```svelte
<Select id="example-select" label="Size" options={["Small", "Large"]} value="Small" />
```

## Combobox

Choose a string option using the native select adapter.

**Props:** `label`, `value`: nonempty strings; `options`: nonempty unique string list; `enabled`: boolean; protocol event prop `on_changed`. Select value must occur in options.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries a selected string.

**Current support:** Native selector is wired; Combobox shares the select adapter and does not promise separate freeform search semantics.

```svelte
<Combobox id="example-combobox" label="Size" options={["Small", "Large"]} value="Small" />
```

## NumberInput

Edit a bounded numeric value.

**Props:** `label`: nonempty string; `value`, `min`, `max`, `step`: finite numbers; `enabled`: boolean; protocol event prop `on_changed`. Use a positive step and consistent bounds.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries a finite numeric value.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<NumberInput id="example-number-input" label="Quantity" value={2} min={0} max={10} step={1} />
```

## TextInput

Edit a single line of non-secret text.

**Props:** `label`: nonempty string; `value`, `placeholder`: strings; `enabled`: boolean; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries non-secret text. bind:value is not admitted by Studio Script.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<TextInput id="example-text-input" label="Notes" placeholder="Enter notes" value="" />
```

## TextArea

Edit multiple lines of non-secret text.

**Props:** `label`: nonempty string; `value`, `placeholder`: strings; `enabled`: boolean; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries non-secret text. bind:value is not admitted by Studio Script.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<TextArea id="example-text-area" label="Notes" placeholder="Enter notes" value="" />
```

## Field

Label a single control and show its description/error.

**Props:** `label`, `description`, `error`: nonempty strings when supplied. Omit error when there is no error.

**Children:** 0–1. **Events/state:** Attach onchange to the nested control; metadata alone does not create a form submission.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<Field id="example-field" label="Email" description="Use your work address">
  <TextInput id="field-input" label="Email" placeholder="name@example.com" />
</Field>
```

## InputGroup

Group one control root with field metadata.

**Props:** `label`, `description`, `error`: nonempty strings when supplied. Omit error when there is no error.

**Children:** 0–1. **Events/state:** Attach onchange to the nested control; metadata alone does not create a form submission.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<InputGroup id="example-input-group" label="Email" description="Use your work address">
  <TextInput id="inputgroup-input" label="Email" placeholder="name@example.com" />
</InputGroup>
```

## OtpInput

Edit a bounded non-secret one-time code.

**Props:** `label`, `value`: strings; `length`: integer 1–12; `enabled`: boolean; protocol event prop `on_changed`.

**Children:** none. **Events/state:** onchange={handler}; native changed payload carries the entered string. This is not the SecretInput contract.

**Current support:** Dedicated native rendering; verify behavior in the dev preview.

```svelte
<OtpInput id="example-otp-input" label="Verification code" length={6} value="" />
```

## SecretInput

Collect host-owned sensitive input without exposing raw bytes to guest code.

**Props:** `label`: nonempty string; `ready`, `enabled`: booleans; protocol event prop `on_ready`. No guest-readable value property.

**Children:** none. **Events/state:** Host emits ready with an opaque authorization_ref. There is no onready syntax mapping in Studio Script.

**Current support:** Never add value or bind:value. A ready prop does not manufacture an authorization reference; verify the host action integration.

```svelte
<SecretInput id="example-secret-input" label="Payment PIN" enabled={true} />
```
