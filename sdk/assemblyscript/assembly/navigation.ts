/** Closed navigation command helpers for protocol v1. */
export class NavigationCommand {
  private constructor(
    public readonly operation: string,
    public readonly route: string = "",
  ) {}

  static push(route: string): NavigationCommand {
    return new NavigationCommand("push", validateRoute(route));
  }

  static replace(route: string): NavigationCommand {
    return new NavigationCommand("replace", validateRoute(route));
  }

  static pop(): NavigationCommand {
    return new NavigationCommand("pop");
  }

  static popTo(route: string): NavigationCommand {
    return new NavigationCommand("pop_to", validateRoute(route));
  }

  static reset(route: string): NavigationCommand {
    return new NavigationCommand("reset", validateRoute(route));
  }

  toJson(): string {
    const routeField = this.route.length == 0 ? "" : ',"route":"' + this.route + '"';
    return '{"type":"navigate","payload":{"operation":"' + this.operation + '"' +
      routeField + "}}";
  }
}

/** One declared route pattern used by a plugin screen registry. */
export class RouteDeclaration {
  public readonly pattern: string;

  constructor(pattern: string) {
    this.pattern = validateRoute(pattern);
  }
}

/** Stable result delivered after the host has atomically handled a command. */
export class NavigationResult {
  constructor(
    public readonly route: string,
    public readonly accepted: boolean,
    public readonly errorCode: string = "",
  ) {}
}

/** Closed guest guard decision. Pending protected flows still require host confirmation. */
export enum NavigationGuardDecision {
  Allow,
  Deny,
}

/** One-at-a-time navigation correlation helper. */
export class NavigationCorrelation {
  currentRoute: string = "";
  private pending: bool = false;

  begin(_command: NavigationCommand): void {
    if (this.pending) throw new Error("navigation already pending");
    this.pending = true;
  }

  resolve(result: NavigationResult): void {
    if (!this.pending) throw new Error("no navigation pending");
    this.pending = false;
    if (result.accepted) this.currentRoute = result.route;
  }
}

function validateRoute(route: string): string {
  if (route.length < 2 || route.charCodeAt(0) != 47 || route.includes("//") ||
      route.includes("?") || route.includes("#") || route.includes("\\")) {
    throw new Error("route must be an absolute safe path");
  }
  return route;
}

/** One registry entry mirrored from the generated route table. */
export class RouteTableEntry {
  constructor(
    public readonly path: string,
    public readonly params: string[],
    public readonly title: string,
    public readonly file: string,
    public readonly kind: string,
  ) {}
}

/** A resolved match: entry plus extracted parameters. */
export class RouteMatch {
  constructor(
    public readonly entry: RouteTableEntry,
    public readonly params: Map<string, string>,
  ) {}
}

function normalizeRoutePath(path: string): string {
  let clean = path;
  const query = clean.indexOf("?");
  if (query >= 0) clean = clean.substring(0, query);
  const fragment = clean.indexOf("#");
  if (fragment >= 0) clean = clean.substring(0, fragment);
  while (clean.length > 1 && clean.endsWith("/")) {
    clean = clean.substring(0, clean.length - 1);
  }
  return clean.length == 0 ? "/" : clean;
}

function splitSegments(path: string): string[] {
  const parts = path.split("/");
  const segments: string[] = [];
  for (let i = 0; i < parts.length; i++) {
    if (parts[i].length > 0) segments.push(parts[i]);
  }
  return segments;
}

/**
 * Match one concrete path against a route table with the shared rule:
 * statics before params before wildcard, parameters captured, miss yields
 * null. Mirrors the Rust registry matcher and the reload route policy.
 */
export function matchRoute(
  table: RouteTableEntry[],
  notFound: string | null,
  path: string,
): RouteMatch | null {
  const request = splitSegments(normalizeRoutePath(path));
  const statics: RouteTableEntry[] = [];
  const params: RouteTableEntry[] = [];
  const wildcards: RouteTableEntry[] = [];
  for (let i = 0; i < table.length; i++) {
    const entry = table[i];
    if (entry.kind == "wildcard") wildcards.push(entry);
    else if (entry.kind == "param") params.push(entry);
    else statics.push(entry);
  }
  const ordered = statics.concat(params).concat(wildcards);
  for (let i = 0; i < ordered.length; i++) {
    const bound = matchEntry(ordered[i], request);
    if (bound != null) return new RouteMatch(ordered[i], bound);
  }
  if (notFound != null) {
    for (let i = 0; i < table.length; i++) {
      if (table[i].path == notFound) {
        return new RouteMatch(table[i], new Map<string, string>());
      }
    }
  }
  return null;
}

function matchEntry(
  entry: RouteTableEntry,
  request: string[],
): Map<string, string> | null {
  const pattern = splitSegments(entry.path);
  const bound = new Map<string, string>();
  for (let i = 0; i < pattern.length; i++) {
    const segment = pattern[i];
    if (segment == "*") return bound;
    if (i >= request.length) return null;
    if (segment.startsWith(":")) {
      bound.set(segment.substring(1), request[i]);
    } else if (segment != request[i]) {
      return null;
    }
  }
  return pattern.length == request.length ? bound : null;
}
