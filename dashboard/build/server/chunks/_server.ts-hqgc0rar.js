// @bun
import {
  addEvent,
  getMachineByDaemonToken,
  setDaemonReady
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/daemon/hello/_server.ts.js
var POST = async ({ request }) => {
  const auth = request.headers.get("authorization") ?? "";
  const token = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
  const machine = getMachineByDaemonToken(token);
  if (!machine)
    return json({
      ok: false,
      error: "Unknown daemon token"
    }, { status: 401 });
  const body = await request.json().catch(() => ({}));
  setDaemonReady(machine.id, true);
  addEvent(machine.id, "daemon", `Daemon v${body.daemon_version ?? "?"} online (${body.os ?? "unknown OS"})`);
  return json({
    ok: true,
    machine_id: machine.id,
    commands: []
  });
};
export {
  POST
};

//# debugId=A2ACF218AC6C5F3B64756E2164756E21
