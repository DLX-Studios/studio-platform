// @bun
import {
  verifySessionValue
} from "./index-d85vpy0n.js";
import {
  isConfigured
} from "./index-jr5xvt5y.js";
import {
  redirect
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/hooks.server.js
function isPublic(path) {
  return path === "/login" || path.startsWith("/api/auth/login") || path === "/setup" || path.startsWith("/api/setup") || path.startsWith("/api/daemon");
}
var handle = async ({ event, resolve }) => {
  const path = event.url.pathname;
  const isApi = path.startsWith("/api/");
  if (!isConfigured()) {
    if (!isPublic(path) && !path.startsWith("/_app")) {
      if (isApi)
        return Response.json({ error: "Setup not complete" }, { status: 409 });
      redirect(302, "/setup");
    }
    return resolve(event);
  }
  if (path === "/setup" || path.startsWith("/api/setup")) {
    if (isApi)
      return Response.json({ error: "Already configured" }, { status: 409 });
    redirect(302, "/machines");
  }
  if (!verifySessionValue(event.cookies.get("sb_session"))) {
    if (!isPublic(path) && !path.startsWith("/_app")) {
      if (isApi)
        return Response.json({ error: "Session expired \u2014 please log in again." }, { status: 401 });
      redirect(302, "/login");
    }
  } else if (path === "/login")
    redirect(302, "/machines");
  return resolve(event);
};
export {
  handle
};

//# debugId=C4C0221FA706061D64756E2164756E21
