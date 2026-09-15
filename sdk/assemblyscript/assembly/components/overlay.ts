import { widget, Widget } from "../widgets";

export function Dialog(id: string, title: string = "", open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "dialog");
  if (title.length > 0) w.title(title);
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function AlertDialog(id: string, title: string = "", message: string = "", open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "alert_dialog");
  if (title.length > 0) w.title(title);
  if (message.length > 0) w.prop("message", message);
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Popover(id: string, open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "popover");
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Sheet(id: string, open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "sheet");
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function BottomSheet(id: string, open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "bottom_sheet");
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Toast(id: string, message: string = ""): Widget {
  let w = widget(id, "toast");
  if (message.length > 0) w.prop("message", message);
  return w;
}

export function Notification(id: string, message: string = ""): Widget {
  let w = widget(id, "notification");
  if (message.length > 0) w.prop("message", message);
  return w;
}

export function Banner(id: string, message: string = ""): Widget {
  let w = widget(id, "banner");
  if (message.length > 0) w.prop("message", message);
  return w;
}

export function ContextMenu(id: string, message: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "context_menu");
  if (message.length > 0) w.prop("message", message);
  if (child !== null) w.child(child);
  return w;
}

export function CommandPalette(id: string, placeholder: string = "", commands: Array<string> = [], open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "command_palette");
  if (placeholder.length > 0) w.placeholder(placeholder);
  if (commands.length > 0) w.options(commands);
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Tooltip(id: string, message: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "tooltip");
  if (message.length > 0) w.prop("message", message);
  if (child !== null) w.child(child);
  return w;
}


