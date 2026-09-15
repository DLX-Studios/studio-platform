// Generated from routes/ — do not edit
export const route_root = "/";
export const route_orders__id = "/orders/:id";
export const route_orders_active = "/orders/active";
export const route_pos = "/pos";
export const declaredRoutes = ["/", "/orders/:id", "/orders/active", "/pos"];
export interface RouteEntry {
  path: string;
  params: string[];
  title: string;
  file: string;
  kind: "static" | "param" | "wildcard";
}
export const routeTable: RouteEntry[] = [
  { path: "/", params: [], title: "Home", file: "routes/index.ts", kind: "static" },
  { path: "/*", params: [], title: "Not Found", file: "routes/[...rest].ts", kind: "wildcard" },
  { path: "/orders/:id", params: ["id"], title: "Id", file: "routes/orders/[id].ts", kind: "param" },
  { path: "/orders/active", params: [], title: "Active", file: "routes/orders/active.ts", kind: "static" },
  { path: "/pos", params: [], title: "Point of Sale", file: "routes/pos.ts", kind: "static" },
];
export const notFoundRoute: string = "/*";
