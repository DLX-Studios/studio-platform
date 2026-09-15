import { g as listSshKeys } from "../../../chunks/doClient.js";
//#region src/routes/ssh-keys/+page.server.ts
var load = async () => {
	try {
		return {
			keys: await listSshKeys(),
			error: null
		};
	} catch (e) {
		return {
			keys: [],
			error: e.message
		};
	}
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map