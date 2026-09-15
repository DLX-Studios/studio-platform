import { widget, Widget } from "../widgets";

export function Box(id: string, children: Array<Widget> = [], padding: i32 = 0): Widget {
  let w = widget(id, "box");
  if (padding > 0) w.padding(padding);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Container(id: string, children: Array<Widget> = [], padding: i32 = 0): Widget {
  return Box(id, children, padding);
}

export function Column(id: string, children: Array<Widget> = [], gap: i32 = 0): Widget {
  let w = widget(id, "column");
  if (gap > 0) w.gap(gap);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Row(id: string, children: Array<Widget> = [], gap: i32 = 0): Widget {
  let w = widget(id, "row");
  if (gap > 0) w.gap(gap);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Stack(id: string, children: Array<Widget> = [], alignment: string = "start"): Widget {
  let w = widget(id, "stack");
  if (alignment.length > 0) w.alignment(alignment);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Grid(id: string, columns: i32 = 2, children: Array<Widget> = [], gap: i32 = 0): Widget {
  let w = widget(id, "grid");
  w.prop("columns", columns.toString());
  if (gap > 0) w.gap(gap);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function ScrollView(id: string, axis: string = "vertical", child: Widget | null = null): Widget {
  let w = widget(id, "scroll_view");
  w.prop("axis", axis);
  if (child !== null) w.child(child);
  return w;
}

export function ListView(id: string, axis: string = "vertical", children: Array<Widget> = [], gap: i32 = 0): Widget {
  let w = widget(id, "list_view");
  w.prop("axis", axis);
  if (gap > 0) w.gap(gap);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Spacer(id: string, size: i32 = 0): Widget {
  let w = widget(id, "spacer");
  if (size > 0) w.prop("size", size.toString());
  return w;
}

export function Divider(id: string, thickness: i32 = 1): Widget {
  let w = widget(id, "divider");
  w.prop("thickness", thickness.toString());
  return w;
}

