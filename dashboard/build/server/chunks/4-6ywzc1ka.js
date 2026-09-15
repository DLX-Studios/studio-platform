// @bun
import {
  analyticsSummary
} from "./index-s4a1m2g5.js";
import"./index-jr5xvt5y.js";
import {
  __export,
  __require
} from "./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/analytics/_page.server.ts.js
var exports__page_server_ts = {};
__export(exports__page_server_ts, {
  load: () => load
});
var load = async ({ depends }) => {
  depends("app:analytics");
  return analyticsSummary();
};

// .svelte-kit/output/server/nodes/4.js
var index = 4;
var component_cache;
var component = async () => component_cache ??= (await import("./_page.svelte-netdzxfp.js")).default;
var server_id = "src/routes/analytics/+page.server.ts";
var imports = ["_app/immutable/nodes/4.DgDOe5kj.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/Cene61J5.js"];
var stylesheets = [];
var fonts = [];
export {
  component,
  fonts,
  imports,
  index,
  exports__page_server_ts as server,
  server_id,
  stylesheets
};

//# debugId=854DB5C2EA6482DB64756E2164756E21
