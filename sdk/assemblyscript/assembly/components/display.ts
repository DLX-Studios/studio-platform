import { widget, Widget } from "../widgets";

export function Text(id: string, value: string = ""): Widget {
  let w = widget(id, "text");
  if (value.length > 0) w.prop("text", value);
  return w;
}

export function Icon(id: string, name: string = ""): Widget {
  let w = widget(id, "icon");
  if (name.length > 0) w.prop("name", name);
  return w;
}

export function Image(id: string, asset: string = "", alt: string = ""): Widget {
  let w = widget(id, "image");
  if (asset.length > 0) w.prop("asset", asset);
  if (alt.length > 0) w.prop("alt", alt);
  return w;
}

export function Card(id: string, child: Widget | null = null, padding: i32 = 0): Widget {
  let w = widget(id, "card");
  if (padding > 0) w.padding(padding);
  if (child !== null) w.child(child);
  return w;
}

export function Badge(id: string, label: string = ""): Widget {
  let w = widget(id, "badge");
  if (label.length > 0) w.label(label);
  return w;
}

export function Tag(id: string, label: string = "", variant: string = "default"): Widget {
  let w = widget(id, "tag");
  if (label.length > 0) w.label(label);
  if (variant.length > 0) w.prop("variant", variant);
  return w;
}

export function Avatar(id: string, fallback: string = "", asset: string = "", alt: string = fallback): Widget {
  let w = widget(id, "avatar");
  if (fallback.length > 0) w.prop("fallback", fallback);
  if (asset.length > 0) w.prop("asset", asset);
  if (alt.length > 0) w.prop("alt", alt);
  return w;
}

export function EmptyState(id: string, title: string = "", description: string = ""): Widget {
  let w = widget(id, "empty");
  if (title.length > 0) w.title(title);
  if (description.length > 0) w.prop("description", description);
  return w;
}

export function Skeleton(id: string, width: i32 = 0, height: i32 = 0): Widget {
  let w = widget(id, "skeleton");
  if (width > 0) w.prop("width", width.toString());
  if (height > 0) w.prop("height", height.toString());
  return w;
}

export function ProgressIndicator(id: string, value: f64 = 0.0): Widget {
  let w = widget(id, "progress_indicator");
  w.prop("value", value.toString());
  return w;
}

export function ProgressCircle(id: string, value: f64 = 0.0): Widget {
  let w = widget(id, "progress_circle");
  w.prop("value", value.toString());
  return w;
}

export function Spinner(id: string, label: string = "Loading"): Widget {
  let w = widget(id, "spinner");
  w.label(label);
  return w;
}

