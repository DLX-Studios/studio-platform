# Contract: Route Table Module

**Status**: Contract for feature 006. Additive-only after landing.

`assembly/routes.generated.ts` keeps `route_<name>` consts and `declaredRoutes`
byte-identical, then appends:

```ts
export interface RouteEntry {
  path: string;      // "/orders/:id"
  params: string[];  // ["id"]
  title: string;     // "Order"
  file: string;      // "routes/orders/[id].ts"
  kind: "static" | "param" | "wildcard";
}
export const routeTable: RouteEntry[] = [ /* sorted by path */ ];
export const notFoundRoute: string | null = null; // or the catch-all path
```

## Matching rule (all implementations)

Normalize (strip query/fragment, trailing slash except root, no case folding), split on
`/`, pair left to right (static equal, `:param` captures one, trailing `*` captures rest).
First registry-order match wins: statics before params before wildcard per level. Miss
yields the not-found entry or explicit miss.
