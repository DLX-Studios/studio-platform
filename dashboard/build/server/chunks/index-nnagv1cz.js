// @bun
import {
  listAllDroplets,
  listDropletsByTag,
  statusFromDroplet,
  sweepOrphanVolumes
} from "./index-mcwdb471.js";
import {
  getMachine,
  listMachines,
  machineTag,
  setDaemonReady
} from "./index-jr5xvt5y.js";

// .svelte-kit/output/server/chunks/machinesStatus.js
var PROBE_PORT = 4545;
var PROBE_TIMEOUT_MS = 2500;
async function probeDaemon(ip, token) {
  if (!ip || !token)
    return { ok: false };
  try {
    const r = await fetch(`http://${ip}:${PROBE_PORT}/probe`, {
      headers: { Authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(PROBE_TIMEOUT_MS)
    });
    if (!r.ok)
      return { ok: false };
    const data = await r.json();
    return {
      ok: !!data,
      data
    };
  } catch {
    return { ok: false };
  }
}
async function probeUntilReady(machine, state, ip) {
  if (state !== "active" && state !== "provisioning")
    return state;
  if (machine.daemon_ready || !machine.daemon_token)
    return machine.daemon_ready ? "ready" : "provisioning";
  if ((await probeDaemon(ip, machine.daemon_token)).ok) {
    setDaemonReady(machine.id, true);
    return "ready";
  }
  return "provisioning";
}
async function machinesWithStatus() {
  sweepOrphanVolumes().catch(() => {});
  const machines = listMachines();
  let droplets = [];
  try {
    droplets = (await listAllDroplets()).droplets;
  } catch {}
  return Promise.all(machines.map(async (m) => {
    const d = droplets.find((droplet) => droplet.tags?.includes(machineTag(m.id)));
    const status = statusFromDroplet(d);
    if (status.state === "active")
      status.state = await probeUntilReady(m, "active", status.ip);
    return {
      ...m,
      status
    };
  }));
}
async function machineWithStatus(id) {
  sweepOrphanVolumes().catch(() => {});
  const machine = getMachine(id);
  if (!machine)
    return {
      machine: null,
      status: {
        running: false,
        state: "off",
        ip: null,
        droplet_id: null
      }
    };
  let status = {
    running: false,
    state: "off",
    ip: null,
    droplet_id: null
  };
  try {
    const data = await listDropletsByTag(machineTag(machine.id));
    status = statusFromDroplet(data.droplets[0]);
  } catch {}
  if (status.state === "active")
    status.state = await probeUntilReady(machine, "active", status.ip);
  return {
    machine,
    status
  };
}

export { machinesWithStatus, machineWithStatus };

//# debugId=676CFD0C33DEE8B064756E2164756E21
