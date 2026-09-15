// Studio template gallery helpers: pure logic for the new-project flow.
//
// This module must never import "vscode" so it stays unit-testable under
// bun:test. All editor interaction lives in extension.ts.

export interface GalleryTemplate {
  id: string;
  kind: string;
  description: string;
}

/** Parse `studio new --list-templates --format json` output. */
export function parseTemplateList(output: string): GalleryTemplate[] {
  const trimmed = output.trim();
  if (trimmed.length === 0) {
    return [];
  }
  const parsed: unknown = JSON.parse(trimmed);
  if (!Array.isArray(parsed)) {
    throw new Error("template list is not an array");
  }
  return parsed.map((entry) => {
    if (
      typeof entry !== "object" ||
      entry === null ||
      typeof (entry as Record<string, unknown>).id !== "string" ||
      typeof (entry as Record<string, unknown>).kind !== "string" ||
      typeof (entry as Record<string, unknown>).description !== "string"
    ) {
      throw new Error("template entry is malformed");
    }
    const record = entry as Record<string, string>;
    return { id: record.id, kind: record.kind, description: record.description };
  });
}

/** Mirror of the CLI slug rule: lowercase alphanumeric plus interior `-`. */
export function isValidProjectName(name: string): boolean {
  return (
    name.length > 0 &&
    name.length <= 64 &&
    /^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$/.test(name)
  );
}

/** Shell-quote one argument for the terminal scaffold command. */
export function shellQuote(argument: string): string {
  if (/^[a-zA-Z0-9_@%+=:,./-]+$/.test(argument) && argument.length > 0) {
    return argument;
  }
  return `'${argument.replace(/'/g, `'\\''`)}'`;
}

/** Build the terminal command scaffolding a project in a directory. */
export function buildNewCommand(
  directory: string,
  name: string,
  template: string,
): string {
  return `cd ${shellQuote(directory)} && studio new ${shellQuote(name)} -t ${shellQuote(template)}`;
}
