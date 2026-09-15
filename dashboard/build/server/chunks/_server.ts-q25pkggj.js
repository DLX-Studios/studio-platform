// @bun
import {
  listDropletsByTag,
  statusFromDroplet
} from "./index-mcwdb471.js";
import {
  getMachine,
  machineTag
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/machines/_id_/agent/_server.ts.js
var DAEMON_PORT = 4545;
async function daemonTarget(id) {
  const machine = getMachine(id);
  if (!machine)
    throw new Error("Machine not found.");
  if (!machine.daemon_token)
    throw new Error("This machine has not been provisioned yet.");
  const droplets = await listDropletsByTag(machineTag(id));
  const status = statusFromDroplet(droplets.droplets[0]);
  if (!status.running || !status.ip)
    throw new Error("The builder is not running.");
  return {
    url: `http://${status.ip}:${DAEMON_PORT}`,
    token: machine.daemon_token
  };
}
var GET = async ({ params, request }) => {
  try {
    const { url, token } = await daemonTarget(Number(params.id));
    const after = request.headers.get("last-event-id") ?? "0";
    const upstream = await fetch(`${url}/agent/events?after=${encodeURIComponent(after)}`, {
      headers: {
        Accept: "text/event-stream",
        Authorization: `Bearer ${token}`
      },
      signal: request.signal
    });
    if (!upstream.ok || !upstream.body) {
      const detail = await upstream.text().catch(() => "");
      return json({ error: detail || `Agent stream returned ${upstream.status}.` }, { status: upstream.status || 502 });
    }
    return new Response(upstream.body, { headers: {
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "Content-Type": "text/event-stream",
      "X-Accel-Buffering": "no"
    } });
  } catch (error) {
    return json({ error: error.message }, { status: 503 });
  }
};
var POST = async ({ params, request }) => {
  try {
    const command = await request.json();
    const { url, token } = await daemonTarget(Number(params.id));
    const upstream = await fetch(`${url}/agent/command`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json"
      },
      body: JSON.stringify(command),
      signal: AbortSignal.timeout(1e4)
    });
    const body = await upstream.json().catch(() => ({}));
    if (!upstream.ok)
      return json({ error: body.error ?? `Agent command returned ${upstream.status}.` }, { status: upstream.status });
    return json(body, { status: upstream.status });
  } catch (error) {
    return json({ error: error.message }, { status: 503 });
  }
};
export {
  GET,
  POST
};

//# debugId=3205E798061813F264756E2164756E21
