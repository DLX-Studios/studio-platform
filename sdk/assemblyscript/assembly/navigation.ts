/** Closed navigation command helpers for protocol v1. */
export class NavigationCommand {
  private constructor(
    public readonly operation: string,
    public readonly route: string = "",
  ) {}

  static push(route: string): NavigationCommand {
    return new NavigationCommand("push", validateConcreteRoute(route));
  }

  static replace(route: string): NavigationCommand {
    return new NavigationCommand("replace", validateConcreteRoute(route));
  }

  static pop(): NavigationCommand {
    return new NavigationCommand("pop");
  }

  static popTo(route: string): NavigationCommand {
    return new NavigationCommand("pop_to", validateConcreteRoute(route));
  }

  static reset(route: string): NavigationCommand {
    return new NavigationCommand("reset", validateConcreteRoute(route));
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
  if (!isAbsoluteSafePath(route)) {
    throw new Error("route must be an absolute safe path");
  }
  return route;
}

function isAbsoluteSafePath(route: string): boolean {
  if (route.length < 1 || route.length > 2048 || route.charCodeAt(0) != 47 ||
      route.includes("//") || route.includes("?") || route.includes("#") || route.includes("\\")) {
    return false;
  }
  for (let i = 0; i < route.length; i++) {
    const code = route.charCodeAt(i);
    if (code < 32 || (code >= 127 && code <= 159)) return false;
  }
  return true;
}

function isConcreteRoute(route: string): boolean {
  if (!isAbsoluteSafePath(route)) return false;
  if (route == "/") return true;
  const segments = route.substring(1).split("/");
  for (let i = 0; i < segments.length; i++) {
    const segment = segments[i];
    if (segment.length == 0 || segment == "." || segment == ".." ||
        segment.startsWith(":") || segment.includes("*")) return false;
  }
  return true;
}

function validateConcreteRoute(route: string): string {
  if (!isConcreteRoute(route)) throw new Error("route must be an absolute safe concrete path");
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
 * compare each segment as static before parameter before wildcard, with an
 * exact ending before an empty catch-all. Parameters are captured and invalid
 * concrete paths cannot resolve. Mirrors the shared Rust route tree.
 */
export function matchRoute(
  table: RouteTableEntry[],
  notFound: string | null,
  path: string,
): RouteMatch | null {
  const normalized = normalizeRoutePath(path);
  if (!isConcreteRoute(normalized)) return null;
  const request = splitSegments(normalized);
  let best: RouteMatch | null = null;
  for (let i = 0; i < table.length; i++) {
    const entry = table[i];
    const bound = matchEntry(entry, request);
    if (bound != null && (best == null || moreSpecific(entry.path, best!.entry.path))) {
      best = new RouteMatch(entry, bound);
    }
  }
  if (best != null) return best;
  if (notFound != null) {
    for (let i = 0; i < table.length; i++) {
      if (table[i].path == notFound) {
        const bound = matchEntry(table[i], request);
        if (bound != null) return new RouteMatch(table[i], bound);
      }
    }
  }
  return null;
}

function moreSpecific(left: string, right: string): boolean {
  const leftSegments = splitSegments(left);
  const rightSegments = splitSegments(right);
  const length = leftSegments.length > rightSegments.length ? leftSegments.length : rightSegments.length;
  for (let i = 0; i <= length; i++) {
    const leftRank = i >= leftSegments.length ? 1 : leftSegments[i] == "*" ? 0 : leftSegments[i].startsWith(":") ? 2 : 3;
    const rightRank = i >= rightSegments.length ? 1 : rightSegments[i] == "*" ? 0 : rightSegments[i].startsWith(":") ? 2 : 3;
    if (leftRank != rightRank) return leftRank > rightRank;
  }
  return false;
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
