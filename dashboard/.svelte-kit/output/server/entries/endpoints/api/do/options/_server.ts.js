import { c as getSetting } from "../../../../../chunks/db.js";
import { d as listDistributions, f as listDropletSnapshots, g as listSshKeys, h as listSizes, m as listRegions, s as describeDoError } from "../../../../../chunks/doClient.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/do/options/+server.ts
/**
* Everything the machine-creation flow needs from DigitalOcean in one call:
* available regions, size catalog (real vCPU/RAM/disk/price), droplet
* snapshots and account SSH keys.
*/
var GET = async () => {
	try {
		const [regions, sizes, snapshots, sshKeys, distributions] = await Promise.all([
			listRegions(),
			listSizes(),
			listDropletSnapshots(),
			listSshKeys(),
			listDistributions()
		]);
		return json({
			regions,
			sizes,
			snapshots,
			sshKeys,
			distributions,
			openrouter_configured: !!getSetting("openrouter_key")
		});
	} catch (e) {
		const d = describeDoError(e);
		return json({ error: d.message }, { status: 502 });
	}
};
//#endregion
export { GET };

//# sourceMappingURL=_server.ts.js.map