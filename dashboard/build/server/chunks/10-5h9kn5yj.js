// @bun
import {
  listSshKeys
} from "./index-mcwdb471.js";
import"./index-jr5xvt5y.js";
import {
  __export,
  __require
} from "./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/ssh-keys/_page.server.ts.js
var exports__page_server_ts = {};
__export(exports__page_server_ts, {
  load: () => load
});
var load = async () => {
  try {
    return {
      keys: await listSshKeys(),
      error: null
    };
  } catch (e) {
    return {
      keys: [],
      error: e.message
    };
  }
};

// .svelte-kit/output/server/nodes/10.js
var index = 10;
var component_cache;
var component = async () => component_cache ??= (await import("./_page.svelte-e46kfxn5.js")).default;
var server_id = "src/routes/ssh-keys/+page.server.ts";
var imports = ["_app/immutable/nodes/10.DcxKhWqh.js", "_app/immutable/chunks/DPBx5xTx.js", "_app/immutable/chunks/Cene61J5.js", "_app/immutable/chunks/CmwU_1OI.js", "_app/immutable/chunks/D4UQy-ZY.js", "_app/immutable/entry/payload.DSmR2FwN.js", "_app/immutable/chunks/BIRBqmYv.js", "_app/immutable/chunks/DjAkF-xo.js", "_app/immutable/chunks/CV8VO5Jt.js", "_app/immutable/chunks/BTmxmbKa.js"];
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

//# debugId=392F819964DBE4C764756E2164756E21
