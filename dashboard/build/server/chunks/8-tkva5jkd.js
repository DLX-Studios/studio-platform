// @bun
import {
  getSetting
} from "./index-jr5xvt5y.js";
import {
  __export,
  __require
} from "./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/providers/_page.server.ts.js
var exports__page_server_ts = {};
__export(exports__page_server_ts, {
  load: () => load
});
var load = async () => {
  return { doConnected: !!getSetting("do_token") };
};

// .svelte-kit/output/server/nodes/8.js
var index = 8;
var component_cache;
var component = async () => component_cache ??= (await import("./_page.svelte-jcj0cp9g.js")).default;
var server_id = "src/routes/providers/+page.server.ts";
var imports = ["_app/immutable/nodes/8.DnbG8dL_.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/Cene61J5.js", "_app/immutable/chunks/CmwU_1OI.js", "_app/immutable/chunks/D4UQy-ZY.js", "_app/immutable/entry/payload.DSmR2FwN.js", "_app/immutable/chunks/BIRBqmYv.js", "_app/immutable/chunks/DjAkF-xo.js", "_app/immutable/chunks/CV8VO5Jt.js", "_app/immutable/chunks/BTmxmbKa.js"];
var stylesheets = ["_app/immutable/assets/OptionCard.I4OQdEuu.css"];
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

//# debugId=77E1532F6034984164756E2164756E21
