import { widget, Widget } from "../widgets";

export function ListTile(id: string, title: string = "", child: Widget | null = null): Widget {
  let w = widget(id, "list_tile");
  if (title.length > 0) w.title(title);
  if (child !== null) w.child(child);
  return w;
}

export function SearchableList(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "searchable_list");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function VirtualList(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "virtual_list");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function DataTable(id: string, columns: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "data_table");
  if (columns.length > 0) w.options(columns);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function Tree(id: string, items: Array<string> = [], children: Array<Widget> = []): Widget {
  let w = widget(id, "tree");
  if (items.length > 0) w.options(items);
  if (children.length > 0) w.addChildren(children);
  return w;
}

export function DescriptionList(id: string, items: Array<string> = []): Widget {
  let w = widget(id, "description_list");
  if (items.length > 0) w.options(items);
  return w;
}

export function Calendar(id: string, value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "calendar");
  if (value.length > 0) w.value(value);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function DatePicker(id: string, value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "date_picker");
  if (value.length > 0) w.value(value);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}

export function TimePicker(id: string, value: string = "", onChanged: string = ""): Widget {
  let w = widget(id, "time_picker");
  if (value.length > 0) w.value(value);
  if (onChanged.length > 0) w.onChanged(onChanged);
  return w;
}


