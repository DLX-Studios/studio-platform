import { describe, expect, test } from "bun:test";
import { RouteTableEntry, matchRoute } from "../assembly/navigation";

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
});
