import { a as getMachine, c as getSetting, t as addEvent } from "./db.js";
import { FetchRequestAdapter } from "@microsoft/kiota-http-fetchlibrary";
//#region src/lib/sdkLoader.ts
/**
* Loader for @digitalocean/dots.
*
* The SDK ships its Kiota-generated *TypeScript sources* (no .d.ts), and its
* sources don't typecheck against the installed Kiota version. Any static
* import (even subpaths — root index.ts exists, so the .js→.ts substitution
* always wins) drags 237+ source errors into our typecheck program.
*
* Solution: import with a computed specifier. TS ignores non-literal dynamic
* imports, and the bundler (rolldown) leaves them for runtime resolution from
* node_modules. Cached after first load.
*/
var cached = null;
function loadDoSdk() {
	if (!cached) cached = import(
		/* @vite-ignore */
		["@digitalocean", "dots"].join("/")
);
	return cached;
}
//#endregion
//#region src/lib/doClient.ts
async function makeClient(tokenOverride) {
	const sdk = await loadDoSdk();
	const token = tokenOverride ?? getSetting("do_token") ?? "";
	const authProvider = new sdk.DigitalOceanApiKeyAuthenticationProvider(token);
	const adapter = new FetchRequestAdapter(authProvider);
	return sdk.createDigitalOceanClient(adapter);
}
function describeDoError(e) {
	const code = e?.responseStatusCode;
	if (code === 401 || code === 403) return {
		reason: "rejected",
		message: `DigitalOcean rejected this Personal Access Token (wrong or revoked, status ${code}). It is not a network error — do not generate a new one unless you are sure this one is dead.`
	};
	if (code === 429) return {
		reason: "rate",
		message: "DigitalOcean is rate-limiting this token. Wait a moment and try again."
	};
	if (typeof code === "number" && code >= 500) return {
		reason: "network",
		message: `DigitalOcean had a server error (status ${code}). Do not generate a new Personal Access Token — this is not your token's fault.`
	};
	return {
		reason: "network",
		message: "Could not reach DigitalOcean. Check your connection — do not generate a new Personal Access Token yet."
	};
}
async function listAllDroplets(tokenOverride) {
	return { droplets: ((await doFetch("/droplets?per_page=200", {}, tokenOverride))?.droplets ?? []).map(toPlainDropletRest) };
}
async function listDropletsByTag(tag, tokenOverride) {
	return { droplets: ((await doFetch(`/droplets?tag_name=${encodeURIComponent(tag)}&per_page=200`, {}, tokenOverride))?.droplets ?? []).map(toPlainDropletRest) };
}
function toPlainDropletRest(d) {
	return {
		id: d?.id ?? 0,
		name: d?.name ?? "",
		status: d?.status ?? "off",
		tags: d?.tags ?? [],
		networks: { v4: (d?.networks?.v4 ?? []).map((n) => ({
			ipAddress: n?.ip_address ?? "",
			type: n?.type ?? ""
		})) },
		sizeSlug: d?.size_slug ?? "",
		region: { slug: d?.region?.slug ?? "" },
		createdAt: d?.created_at ?? null
	};
}
async function createDroplet(body, tokenOverride) {
	const d = (await doFetch("/droplets", {
		method: "POST",
		body: JSON.stringify({
			name: body.name,
			region: body.region,
			size: body.size,
			image: body.image,
			ssh_keys: body.ssh_keys,
			tags: body.tags,
			user_data: body.user_data,
			monitoring: body.monitoring ?? false,
			ipv6: false,
			backups: false,
			...body.dedicatedCpu ? { dedicated_cpu: true } : {},
			...body.volumes?.length ? { volumes: body.volumes } : {}
		})
	}, tokenOverride))?.droplet;
	if (!d) throw new Error("DO returned no droplet in create response");
	return { droplet: toPlainDropletRest(d) };
}
async function deleteDroplet(id) {
	await (await makeClient()).v2.droplets.byDroplet_id(id).delete();
	return null;
}
/** Validate a token — throws with responseStatusCode on rejection. */
async function getAccount(tokenOverride) {
	return (await makeClient(tokenOverride)).v2.account.get();
}
async function listSshKeys(tokenOverride) {
	return ((await (await makeClient(tokenOverride)).v2.account.keys.get())?.sshKeys ?? []).map((k) => ({
		id: k?.id ?? 0,
		name: k?.name ?? "",
		fingerprint: k?.fingerprint ?? ""
	}));
}
async function listDropletSnapshots(tokenOverride) {
	return ((await (await makeClient(tokenOverride)).v2.snapshots.get({
		resourceType: "droplet",
		perPage: 200
	}))?.snapshots ?? []).map((s) => ({
		id: String(s?.id ?? ""),
		name: s?.name ?? ""
	}));
}
/**
* Regions & sizes go through plain REST: the dynamically-loaded SDK fails
* to deserialize these two responses ("unsupported type during
* deserialization"), while plain fetch + our own mapping works fine.
*/
async function listRegions(tokenOverride) {
	return ((await doFetch("/regions?per_page=200", {}, tokenOverride))?.regions ?? []).filter((r) => r?.available).map((r) => ({
		slug: r?.slug ?? "",
		name: r?.name ?? r?.slug ?? ""
	}));
}
async function listSizes(tokenOverride) {
	return ((await doFetch("/sizes?per_page=200", {}, tokenOverride))?.sizes ?? []).map((s) => ({
		slug: s?.slug ?? "",
		vcpus: s?.vcpus ?? 0,
		memoryGb: Math.round((s?.memory ?? 0) / 1024),
		diskGb: s?.disk ?? 0,
		hourly: s?.price_hourly ?? 0,
		monthly: s?.price_monthly ?? 0,
		regions: s?.regions ?? [],
		available: s?.available ?? false,
		description: s?.description ?? ""
	}));
}
function doFetch(path, init = {}, tokenOverride) {
	const token = tokenOverride ?? getSetting("do_token") ?? "";
	return fetch(`https://api.digitalocean.com/v2${path}`, {
		...init,
		headers: {
			Authorization: `Bearer ${token}`,
			"Content-Type": "application/json",
			...init?.headers ?? {}
		}
	}).then(async (r) => {
		const data = await r.json().catch(() => ({}));
		if (!r.ok) throw Object.assign(new Error(data?.message ?? `DO request failed (${r.status})`), { responseStatusCode: r.status });
		return data;
	});
}
async function listDistributions(tokenOverride) {
	return ((await doFetch("/images?type=distribution&per_page=200", {}, tokenOverride))?.images ?? []).filter((i) => typeof i?.slug === "string" && i.slug.endsWith("-x64") && !i.slug.startsWith("gpu-")).map((i) => ({
		id: String(i?.id ?? ""),
		slug: i?.slug ?? "",
		name: i?.name ?? "",
		distribution: i?.distribution ?? ""
	}));
}
/** Create an SSH key on the DO account (wizard: "paste a public key"). */
async function createSshKey(name, publicKey, tokenOverride) {
	const data = await doFetch("/account/keys", {
		method: "POST",
		body: JSON.stringify({
			name,
			public_key: publicKey
		})
	}, tokenOverride);
	return {
		id: data?.ssh_key?.id ?? 0,
		name: data?.ssh_key?.name ?? "",
		fingerprint: data?.ssh_key?.fingerprint ?? ""
	};
}
/** Queue a snapshot of a (running) droplet. DO processes it asynchronously. */
async function createDropletSnapshot(dropletId, name) {
	return { actionId: (await doFetch(`/droplets/${dropletId}/actions`, {
		method: "POST",
		body: JSON.stringify({
			type: "snapshot",
			name
		})
	}))?.action?.id ?? 0 };
}
async function listVolumesByName(name) {
	return ((await doFetch(`/volumes?name=${encodeURIComponent(name)}`))?.volumes ?? []).map((v) => ({
		id: v?.id ?? 0,
		region: v?.region?.slug ?? "",
		dropletIds: v?.droplet_ids ?? []
	}));
}
/** Detach a volume from a droplet (no-op if already detached). */
async function detachVolume(id, dropletId) {
	await doFetch(`/volumes/${id}?droplet_id=${dropletId}`, { method: "DELETE" });
	return null;
}
async function createVolume(args) {
	return { id: (await doFetch("/volumes", {
		method: "POST",
		body: JSON.stringify({
			name: args.name,
			region: args.region,
			size_gigabytes: args.sizeGb,
			filesystem_type: "ext4"
		})
	}))?.volume?.id ?? 0 };
}
async function deleteVolume(id) {
	await doFetch(`/volumes/${id}`, { method: "DELETE" });
	return null;
}
var lastSweep = 0;
/**
* Safety net: delete volumes named sb-machine-<id>-data whose machine
* profile no longer exists (e.g. leaked when a delete's DO cleanup failed).
* Throttled to once per 5 minutes; safe to call on every status poll.
*/
async function sweepOrphanVolumes() {
	if (Date.now() - lastSweep < 3e5) return 0;
	lastSweep = Date.now();
	let deleted = 0;
	try {
		const data = await doFetch("/volumes?per_page=200");
		for (const v of data?.volumes ?? []) {
			const match = /^sb-machine-(\d+)-data$/.exec(v?.name ?? "");
			if (!match) continue;
			const machineId = Number(match[1]);
			if (getMachine(machineId)) continue;
			const attached = (v.droplet_ids ?? [])[0];
			const url = attached ? `/volumes/${v.id}?droplet_id=${attached}` : `/volumes/${v.id}`;
			try {
				await doFetch(url, { method: "DELETE" });
				deleted++;
				addEvent(machineId, "destroy", `Orphan volume ${v.name} deleted (profile gone)`);
			} catch {}
		}
	} catch {}
	return deleted;
}
function statusFromDroplet(d) {
	if (!d || d.id === 0) return {
		running: false,
		state: "off",
		ip: null,
		droplet_id: null
	};
	const ip = d.networks.v4.find((n) => n.type === "public")?.ipAddress ?? null;
	return {
		running: d.status === "new" || d.status === "active",
		state: d.status === "new" ? "starting" : d.status,
		ip,
		droplet_id: d.id
	};
}
function sshKeyIds() {
	return (getSetting("do_ssh_key_ids") ?? "").split(",").filter(Boolean).map(Number);
}
//#endregion
export { listVolumesByName as _, deleteDroplet as a, sweepOrphanVolumes as b, detachVolume as c, listDistributions as d, listDropletSnapshots as f, listSshKeys as g, listSizes as h, createVolume as i, getAccount as l, listRegions as m, createDropletSnapshot as n, deleteVolume as o, listDropletsByTag as p, createSshKey as r, describeDoError as s, createDroplet as t, listAllDroplets as u, sshKeyIds as v, statusFromDroplet as y };

//# sourceMappingURL=doClient.js.map