import { c as getSetting } from "../../../chunks/db.js";
//#region src/routes/providers/+page.server.ts
var load = async () => {
	return { doConnected: !!getSetting("do_token") };
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map