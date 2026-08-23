import type { Widget } from "./runtime";

/** One fine-grained property binding attached to a stable widget. */
export interface PropertyBinding {
  readonly property: string;
  readonly read: () => unknown;
}

const bindings = new WeakMap<Widget, Map<string, PropertyBinding>>();

/** Bind one serializable widget property to a reactive getter. */
export function bindProp(
  widget: Widget,
  property: string,
  read: () => unknown,
): Widget {
  if (property.length === 0) throw new Error("binding property must not be empty");
  let widgetBindings = bindings.get(widget);
  if (widgetBindings === undefined) {
    widgetBindings = new Map();
    bindings.set(widget, widgetBindings);
  }
  widgetBindings.set(property, { property, read });
  return widget;
}

/** Return bindings in deterministic property insertion order. */
export function bindingsFor(widget: Widget): PropertyBinding[] {
  return Array.from(bindings.get(widget)?.values() ?? []);
}
