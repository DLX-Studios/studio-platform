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
