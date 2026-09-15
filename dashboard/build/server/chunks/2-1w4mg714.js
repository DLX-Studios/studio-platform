// @bun
import {
  redirect
} from "./index-pcpfewry.js";
import {
  __export,
  __require
} from "./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/_page.ts.js
var exports__page_ts = {};
__export(exports__page_ts, {
  load: () => load
});
var load = () => {
  redirect(302, "/machines");
};

// .svelte-kit/output/server/nodes/2.js
var index = 2;
var component_cache;
var component = async () => component_cache ??= (await import("./_page.svelte-c37k8z4g.js")).default;
var universal_id = "src/routes/+page.ts";
var imports = ["_app/immutable/nodes/2.DGaNZwx9.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/CV8VO5Jt.js"];
var stylesheets = [];
var fonts = [];
export {
  component,
  fonts,
  imports,
  index,
  stylesheets,
  exports__page_ts as universal,
  universal_id
};

//# debugId=AD5360E68C95D14A64756E2164756E21
