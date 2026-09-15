import { f as listDropletSnapshots, g as listSshKeys, l as getAccount, s as describeDoError } from "../../../../../chunks/doClient.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/setup/probe/+server.ts
var POST = async ({ request }) => {
	const { token } = await request.json();
	if (!token) return json({
		error: "Token required",
		reason: "rejected"
	}, { status: 400 });
	try {
		await getAccount(token);
	} catch (e) {
		const d = describeDoError(e);
		return json({
			error: d.message,
			reason: d.reason
		}, { status: 400 });
	}
	const keys = await listSshKeys(token).catch(() => []);
	const snapshots = await listDropletSnapshots(token).catch(() => []);
	return json({
		ok: true,
		keys,
		snapshots
	});
};
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map