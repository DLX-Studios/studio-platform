import { o as getMachineByDaemonToken, p as setDaemonReady, t as addEvent } from "../../../../../chunks/db.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/daemon/hello/+server.ts
var POST = async ({ request }) => {
	const auth = request.headers.get("authorization") ?? "";
	const token = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
	const machine = getMachineByDaemonToken(token);
	if (!machine) return json({
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
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map