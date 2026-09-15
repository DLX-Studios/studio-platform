import { widget, Widget } from "../widgets";

export function Separator(id: string): Widget { return widget(id, "separator"); }

export function Accordion(id: string, title: string = "", open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "accordion");
  if (title.length > 0) w.title(title);
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Collapsible(id: string, open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "collapsible");
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function HoverCard(id: string, child: Widget | null = null): Widget {
  let w = widget(id, "hover_card");
  if (child !== null) w.child(child);
  return w;
}

export function MenuBar(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "menu_bar");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function StatusBar(id: string, value: string = ""): Widget {
  let w = widget(id, "status_bar");
  if (value.length > 0) w.value(value);
  return w;
}

export function KeyboardShortcuts(id: string, keys: Array<string> = []): Widget {
  let w = widget(id, "keyboard_shortcuts");
  if (keys.length > 0) w.options(keys);
  return w;
}

export function Kbd(id: string, value: string = ""): Widget {
  let w = widget(id, "kbd");
  if (value.length > 0) w.value(value);
  return w;
}

export function ColorPicker(id: string, value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "color_picker");
  if (value.length > 0) w.value(value);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Rating(id: string, value: f64 = 0.0, min: f64 = 0.0, max: f64 = 5.0, onChanged: string = ""): Widget {
  let w = widget(id, "rating");
  w.prop("value", value.toString());
  w.prop("min", min.toString());
  w.prop("max", max.toString());
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Resizable(id: string, children: Array<Widget> = []): Widget {
  let w = widget(id, "resizable");
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Dock(id: string, children: Array<Widget> = []): Widget {
  let w = widget(id, "dock");
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Chart(id: string, series: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "chart");
  if (series.length > 0) w.options(series);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Editor(id: string, content: string = "", children: Array<Widget> = []): Widget {
  let w = widget(id, "editor");
  if (content.length > 0) w.prop("content", content);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function RichText(id: string, content: string = ""): Widget {
  let w = widget(id, "rich_text");
  if (content.length > 0) w.prop("content", content);
  return w;
}

export function Carousel(id: string, children: Array<Widget> = []): Widget {
  let w = widget(id, "carousel");
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function DragDrop(id: string, children: Array<Widget> = [], onDrop: string = ""): Widget {
  let w = widget(id, "drag_drop");
  if (onDrop.length > 0) w.prop("on_drop", onDrop);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Theme(id: string, value: string = ""): Widget {
  let w = widget(id, "theme");
  if (value.length > 0) w.value(value);
  return w;
}

export function AspectRatio(id: string, value: f64 = 1.0, child: Widget | null = null): Widget {
  let w = widget(id, "aspect_ratio");
  w.prop("value", value.toString());
  if (child !== null) w.child(child);
  return w;
}

export function Alert(id: string, title: string = "", content: string = ""): Widget {
  let w = widget(id, "alert");
  if (title.length > 0) w.title(title);
  if (content.length > 0) w.prop("content", content);
  return w;
}

export function Attachment(id: string, value: string = ""): Widget {
  let w = widget(id, "attachment");
  if (value.length > 0) w.value(value);
  return w;
}

export function Bubble(id: string, content: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "bubble");
  if (content.length > 0) w.prop("content", content);
  if (child !== null) w.child(child);
  return w;
}

export function Command(id: string, value: string = ""): Widget {
  let w = widget(id, "command");
  if (value.length > 0) w.value(value);
  return w;
}

export function NativeSelect(id: string, value: string = "", options: Array<string> = []): Widget {
  let w = widget(id, "native_select");
  if (value.length > 0) w.value(value);
  if (options.length > 0) w.options(options);
  return w;
}

export function NavigationMenu(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "navigation_menu");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function ScrollArea(id: string, child: Widget | null = null): Widget {
  let w = widget(id, "scroll_area");
  if (child !== null) w.child(child);
  return w;
}

export function Item(id: string, title: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "item");
  if (title.length > 0) w.title(title);
  if (child !== null) w.child(child);
  return w;
}

export function Message(id: string, content: string = ""): Widget {
  let w = widget(id, "message");
  if (content.length > 0) w.prop("content", content);
  return w;
}

export function MessageScroller(id: string, children: Array<Widget> = []): Widget {
  let w = widget(id, "message_scroller");
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function ToggleGroup(id: string, items: Array<string> = [], value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "toggle_group");
  if (items.length > 0) w.options(items);
  if (value.length > 0) w.value(value);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function Sonner(id: string, content: string = ""): Widget {
  let w = widget(id, "sonner");
  if (content.length > 0) w.prop("content", content);
  return w;
}


