import { a as getMachine, f as machineTag, p as setDaemonReady, u as listMachines } from "./db.js";
import { b as sweepOrphanVolumes, p as listDropletsByTag, u as listAllDroplets, y as statusFromDroplet } from "./doClient.js";
//#region src/lib/server/daemonProbe.ts
/**
* Pull-mode daemon probe: the dashboard (possibly behind NAT) polls the
* machine's public IP; the daemon serves a token-authenticated /probe
* endpoint. A 200 with ok:true flips the machine to "ready".
*
* Dial-home remains supported for when the dashboard is publicly reachable.
*/
var PROBE_PORT = 4545;
var PROBE_TIMEOUT_MS = 2500;
async function probeDaemon(ip, token) {
	if (!ip || !token) return { ok: false };
	try {
		const r = await fetch(`http://${ip}:${PROBE_PORT}/probe`, {
			headers: { Authorization: `Bearer ${token}` },
			signal: AbortSignal.timeout(PROBE_TIMEOUT_MS)
		});
		if (!r.ok) return { ok: false };
		const data = await r.json();
		return {
			ok: !!data,
			data
		};
	} catch {
		return { ok: false };
	}
}
/**
* If the machine is booting/provisioning with a known IP, probe it and mark
* it ready when the daemon answers. Returns the (possibly updated) state.
*/
async function probeUntilReady(machine, state, ip) {
	if (state !== "active" && state !== "provisioning") return state;
	if (machine.daemon_ready || !machine.daemon_token) return machine.daemon_ready ? "ready" : "provisioning";
	if ((await probeDaemon(ip, machine.daemon_token)).ok) {
		setDaemonReady(machine.id, true);
		return "ready";
	}
	return "provisioning";
}
//#endregion
//#region src/lib/server/machinesStatus.ts
/**
* Shared machine-status aggregation — used by GET /api/machines, GET
* /api/machines/[id] and the page server loads so all return identical,
* SSR-friendly data.
*/
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
		if (status.state === "active") status.state = await probeUntilReady(m, "active", status.ip);
		return {
			...m,
			status
		};
	}));
}
async function machineWithStatus(id) {
	sweepOrphanVolumes().catch(() => {});
	const machine = getMachine(id);
	if (!machine) return {
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
	if (status.state === "active") status.state = await probeUntilReady(machine, "active", status.ip);
	return {
		machine,
		status
	};
}
//#endregion
export { machinesWithStatus as n, machineWithStatus as t };

//# sourceMappingURL=machinesStatus.js.map