import { c as getSetting, n as createMachine, t as addEvent } from "../../../../chunks/db.js";
import "../../../../chunks/doClient.js";
import { n as machinesWithStatus } from "../../../../chunks/machinesStatus.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/machines/+server.ts
var GET = async () => {
	return json({ machines: await machinesWithStatus() });
};
var POST = async ({ request }) => {
	const body = await request.json();
	if (!body.name?.trim()) return json({ error: "Name required" }, { status: 400 });
	const machine = createMachine({
		name: body.name.trim(),
		size: body.size || getSetting("size") || "c-8",
		region: body.region || getSetting("region") || "nyc3",
		snapshot_id: body.snapshotId || "",
		image: body.imageSlug || "",
		repo: body.repo || null,
		idle_timeout_min: body.idleTimeoutMin ?? 0,
		dedicated: body.dedicated ?? false,
		volume_gb: body.volumeGb ?? 0,
		ssh_key_ids: body.sshKeyIds ?? ""
	});
	addEvent(machine.id, "machine", `Profile "${machine.name}" created`);
	return json({ machine }, { status: 201 });
};
//#endregion
export { GET, POST };

//# sourceMappingURL=_server.ts.js.map