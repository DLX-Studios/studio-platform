import { describe, expect, test } from "bun:test";
import { NavigationCommand, RouteDeclaration, RouteTableEntry, matchRoute } from "../assembly/navigation";

const table: RouteTableEntry[] = [
  new RouteTableEntry("/", [], "Home", "routes/index.ts", "static"),
  new RouteTableEntry("/*", [], "Not Found", "routes/[...rest].ts", "wildcard"),
  new RouteTableEntry("/orders/:id", ["id"], "Id", "routes/orders/[id].ts", "param"),
  new RouteTableEntry("/orders/active", [], "Active", "routes/orders/active.ts", "static"),
  new RouteTableEntry("/pos", [], "Point of Sale", "routes/pos.ts", "static"),
];

describe("route table matching", () => {
  test("static, nested, param, wildcard, and miss semantics", () => {
    const cases: Array<[string, string | null, Record<string, string>]> = [
      ["/", "/", {}],
      ["/pos", "/pos", {}],
      ["/pos/", "/pos", {}],
      ["/pos?tab=1#top", "/pos", {}],
      ["/POS", "/*", {}],
      ["/orders/active", "/orders/active", {}],
      ["/orders/123", "/orders/:id", { id: "123" }],
      ["/orders/abc-def_9", "/orders/:id", { id: "abc-def_9" }],
      ["/orders", "/*", {}],
      ["/orders/1/2", "/*", {}],
      ["/anything/at/all", "/*", {}],
      ["/missing", "/*", {}],
    ];
    for (const [path, wantPath, wantParams] of cases) {
      const found = matchRoute(table, "/*", path);
      expect(found !== null, `match for ${path}`).toBe(true);
      expect(found!.entry.path).toBe(wantPath);
      expect(Object.fromEntries(found!.params)).toEqual(wantParams);
    }
  });

  test("explicit miss without a catch-all", () => {
    const bare = table.filter((entry) => entry.kind == "static");
    expect(matchRoute(bare, null, "/missing")).toBeNull();
    expect(matchRoute(bare, null, "/pos")!.entry.path).toBe("/pos");
  });

  test("precedence follows the first differing segment and exact endings", () => {
    const overlap = [
      new RouteTableEntry("/:section/new", ["section"], "Generic", "routes/[section]/new.ts", "param"),
      new RouteTableEntry("/streams/:id", ["id"], "Stream", "routes/streams/[id].ts", "param"),
    ];
    expect(matchRoute(overlap, null, "/streams/new")!.entry.path).toBe("/streams/:id");
    expect(Object.fromEntries(matchRoute(overlap, null, "/streams/new")!.params)).toEqual({ id: "new" });

    const wildcard = [
      overlap[0],
      new RouteTableEntry("/streams/*", [], "Stream fallback", "routes/streams/[...rest].ts", "wildcard"),
    ];
    expect(matchRoute(wildcard, "/streams/*", "/streams/new")!.entry.path).toBe("/streams/*");
    wildcard.push(new RouteTableEntry("/streams", [], "Streams", "routes/streams.ts", "static"));
    expect(matchRoute(wildcard, "/streams/*", "/streams")!.entry.path).toBe("/streams");
  });

  test("malformed concrete paths cannot resolve even through not-found", () => {
    for (const path of ["/a//b", "/a/../b", "/a/./b", "/a/:id", "/a/*", "relative", "/a\\b", "/a\u0000b", "/a\u0085b"]) {
      expect(matchRoute(table, "/*", path), path).toBeNull();
    }
    expect(matchRoute(table, "/*", "/pos/?tab=1#top")!.entry.path).toBe("/pos");
    expect(matchRoute(table, "/*", "")!.entry.path).toBe("/");
  });

  test("nested catch-all pages only match paths below their declared prefix", () => {
    const nested = [new RouteTableEntry("/streams/*", [], "Fallback", "routes/streams/[...rest].ts", "wildcard")];
    expect(matchRoute(nested, "/streams/*", "/streams/42")!.entry.path).toBe("/streams/*");
    expect(matchRoute(nested, "/streams/*", "/outside")).toBeNull();
  });

  test("root declarations and host navigation commands are valid", () => {
    expect(new RouteDeclaration("/").pattern).toBe("/");
    expect(JSON.parse(NavigationCommand.push("/").toJson())).toEqual({ type: "navigate", payload: { operation: "push", route: "/" } });
    expect(JSON.parse(NavigationCommand.replace("/").toJson()).payload.route).toBe("/");
    expect(JSON.parse(NavigationCommand.reset("/").toJson()).payload.route).toBe("/");
    expect(JSON.parse(NavigationCommand.popTo("/").toJson()).payload.route).toBe("/");
    for (const path of ["/a//b", "/a/../b", "/a/./b", "/a/:id", "/a/*", "/a\u0000b", "/a\u0085b"]) {
      expect(() => NavigationCommand.push(path), path).toThrow();
    }
  });
});
