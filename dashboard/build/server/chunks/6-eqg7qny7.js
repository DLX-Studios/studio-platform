// @bun
import {
  machineWithStatus
} from "./index-nnagv1cz.js";
import"./index-mcwdb471.js";
import"./index-jr5xvt5y.js";
import {
  __export,
  __require
} from "./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/m/_id_/_page.server.ts.js
var exports__page_server_ts = {};
__export(exports__page_server_ts, {
  load: () => load
});
var load = async ({ params, depends }) => {
  const id = Number(params.id);
  depends(`app:machine:${id}`);
  return machineWithStatus(id);
};

// .svelte-kit/output/server/nodes/6.js
var index = 6;
var component_cache;
var component = async () => component_cache ??= (await import("./_page.svelte-h6h7bdy2.js")).default;
var server_id = "src/routes/m/[id]/+page.server.ts";
var imports = ["_app/immutable/nodes/6.D5wrDmNj.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/D4UQy-ZY.js", "_app/immutable/entry/payload.DSmR2FwN.js", "_app/immutable/chunks/BIRBqmYv.js", "_app/immutable/chunks/DjAkF-xo.js", "_app/immutable/chunks/CV8VO5Jt.js", "_app/immutable/chunks/Cene61J5.js"];
var stylesheets = ["_app/immutable/assets/6.CAqGrTQO.css"];
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

//# debugId=00C735F8284EB3DC64756E2164756E21
