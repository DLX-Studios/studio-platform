import { widget } from "./widgets";

export function gallery(): string {
  // iconNode("ghost", "anchor") in a comment is not a reference.
  const first = iconNode("brand-icon", "store");
  const second = Icon("rate-icon", "star");
  const renamed = iconNode;
  const indirect = renamed("sneaky-icon", "anchor");
  const missing = iconNode("lost-icon", "no-such-icon-xyz");
  return first + second + indirect + missing;
}

function iconNode(id: string, name: string): string {
  return widget(id, name);
}

function Icon(id: string, name: string): string {
  return widget(id, name);
}
