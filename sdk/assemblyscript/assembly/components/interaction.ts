import { widget, Widget } from "../widgets";

export function Button(id: string, label: string = "Button", onPressed: string = ""): Widget {
  let w = widget(id, "button");
  w.label(label);
  w.enabled(true);
  if (onPressed.length > 0) w.onPressed(onPressed);
  return w;
}

export function IconButton(id: string, icon: string = "", onPressed: string = ""): Widget {
  let w = widget(id, "icon_button");
  if (icon.length > 0) w.prop("icon", icon);
  w.enabled(true);
  if (onPressed.length > 0) w.onPressed(onPressed);
  return w;
}

export function Checkbox(id: string, label: string = "", value: bool = false, onChanged: string = ""): Widget {
  let w = widget(id, "checkbox");
  if (label.length > 0) w.label(label);
  w.prop("value", value ? "true" : "false");
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Radio(id: string, label: string = "", value: bool = false, onChanged: string = ""): Widget {
  let w = widget(id, "radio");
  if (label.length > 0) w.label(label);
  w.prop("value", value ? "true" : "false");
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function SwitchControl(id: string, label: string = "", value: bool = false, onChanged: string = ""): Widget {
  let w = widget(id, "switch");
  if (label.length > 0) w.label(label);
  w.prop("value", value ? "true" : "false");
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Toggle(id: string, label: string = "", value: bool = false, onChanged: string = ""): Widget {
  let w = widget(id, "toggle");
  if (label.length > 0) w.label(label);
  w.prop("value", value ? "true" : "false");
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function ButtonGroup(id: string, children: Array<Widget> = [], orientation: string = "horizontal"): Widget {
  let w = widget(id, "button_group");
  w.prop("orientation", orientation);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Slider(
  id: string, label: string = "", min: f64 = 0.0, max: f64 = 100.0, value: f64 = 0.0, onChanged: string = "",
): Widget {
  let w = widget(id, "slider");
  if (label.length > 0) w.label(label);
  w.prop("min", min.toString());
  w.prop("max", max.toString());
  w.prop("value", value.toString());
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function RangeSlider(
  id: string, label: string = "", min: f64 = 0.0, max: f64 = 100.0, start: f64 = 0.0, end: f64 = 100.0, onChanged: string = "",
): Widget {
  let w = widget(id, "range_slider");
  if (label.length > 0) w.label(label);
  w.prop("min", min.toString());
  w.prop("max", max.toString());
  w.prop("start", start.toString());
  w.prop("end", end.toString());
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Select(
  id: string, label: string = "", value: string = "", options: Array<string> = [], onChanged: string = "",
): Widget {
  let w = widget(id, "select");
  if (label.length > 0) w.label(label);
  if (value.length > 0) w.value(value);
  if (options.length > 0) w.options(options);
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Combobox(
  id: string, label: string = "", value: string = "", options: Array<string> = [], onChanged: string = "",
): Widget {
  let w = widget(id, "combobox");
  if (label.length > 0) w.label(label);
  if (value.length > 0) w.value(value);
  if (options.length > 0) w.options(options);
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function NumberInput(
  id: string, label: string = "", value: f64 = 0.0, min: f64 = 0.0, max: f64 = 100.0, step: f64 = 1.0, onChanged: string = "",
): Widget {
  let w = widget(id, "number_input");
  if (label.length > 0) w.label(label);
  w.prop("value", value.toString());
  w.prop("min", min.toString());
  w.prop("max", max.toString());
  w.prop("step", step.toString());
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function TextInput(id: string, label: string = "", value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "text_input");
  if (label.length > 0) w.label(label);
  if (value.length > 0) w.value(value);
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function TextArea(id: string, label: string = "", value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "text_area");
  if (label.length > 0) w.label(label);
  if (value.length > 0) w.value(value);
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Field(id: string, label: string = "", description: string = "", error: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "field");
  if (label.length > 0) w.label(label);
  if (description.length > 0) w.prop("description", description);
  if (error.length > 0) w.prop("error", error);
  if (child !== null) w.child(child);
  return w;
}

export function InputGroup(id: string, label: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "input_group");
  if (label.length > 0) w.label(label);
  if (child !== null) w.child(child);
  return w;
}

export function OtpInput(id: string, label: string = "", length: i32 = 6, value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "otp_input");
  if (label.length > 0) w.label(label);
  w.prop("length", length.toString());
  if (value.length > 0) w.value(value);
  w.enabled(true);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function SecretInput(id: string, label: string = "", onReady: string = ""): Widget {
  let w = widget(id, "secret_input");
  if (label.length > 0) w.label(label);
  w.prop("ready", "false");
  w.enabled(true);
  if (onReady.length > 0) w.onReady(onReady);
  return w;
}

