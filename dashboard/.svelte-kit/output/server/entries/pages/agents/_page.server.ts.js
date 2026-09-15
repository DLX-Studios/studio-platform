import { c as getSetting } from "../../../chunks/db.js";
//#region src/routes/agents/+page.server.ts
var load = async () => {
	return { openrouter: !!getSetting("openrouter_key") };
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map