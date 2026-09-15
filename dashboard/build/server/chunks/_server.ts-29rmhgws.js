// @bun
import {
  SESSION_COOKIE
} from "./index-d85vpy0n.js";
import"./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/auth/logout/_server.ts.js
var POST = async ({ cookies }) => {
  cookies.delete(SESSION_COOKIE, { path: "/" });
  return json({ ok: true });
};
export {
  POST
};

//# debugId=8A0B3836282A710664756E2164756E21
