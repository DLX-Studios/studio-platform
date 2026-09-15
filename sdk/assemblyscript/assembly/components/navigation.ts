import { widget, Widget } from "../widgets";

export function Scaffold(id: string, children: Array<Widget> = []): Widget {
  let w = widget(id, "scaffold");
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function AppBar(id: string, title: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "app_bar");
  if (title.length > 0) w.title(title);
  if (child !== null) w.child(child);
  return w;
}

export function Sidebar(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "sidebar");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function NavigationBar(id: string, items: Array<string> = [], value: string = "", onNavigate: string = ""): Widget {
  let w = widget(id, "navigation_bar");
  if (items.length > 0) w.options(items);
  if (value.length > 0) w.value(value);
  if (onNavigate.length > 0) w.onNavigate(onNavigate);
  return w;
}

export function NavigationRail(id: string, items: Array<string> = [], value: string = "", onNavigate: string = ""): Widget {
  let w = widget(id, "navigation_rail");
  if (items.length > 0) w.options(items);
  if (value.length > 0) w.value(value);
  if (onNavigate.length > 0) w.onNavigate(onNavigate);
  return w;
}

export function Drawer(id: string, open: bool = false, child: Widget | null = null): Widget {
  let w = widget(id, "drawer");
  w.open(open);
  if (child !== null) w.child(child);
  return w;
}

export function Tabs(id: string, items: Array<string> = [], value: string = "", children: Array<Widget> = []): Widget {
  let w = widget(id, "tabs");
  if (items.length > 0) w.options(items);
  if (value.length > 0) w.value(value);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Breadcrumb(id: string, items: Array<string> = [], onNavigate: string = ""): Widget {
  let w = widget(id, "breadcrumb");
  if (items.length > 0) w.options(items);
  if (onNavigate.length > 0) w.onNavigate(onNavigate);
  return w;
}

export function Stepper(id: string, step: i32 = 0, children: Array<Widget> = []): Widget {
  let w = widget(id, "stepper");
  w.prop("step", step.toString());
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Pagination(id: string, page: i32 = 1, pages: i32 = 1, onNavigate: string = ""): Widget {
  let w = widget(id, "pagination");
  w.prop("page", page.toString());
  w.prop("pages", pages.toString());
  if (onNavigate.length > 0) w.onNavigate(onNavigate);
  return w;
}


