import { n as machinesWithStatus } from "../../../chunks/machinesStatus.js";
//#region src/routes/machines/+page.server.ts
var load = async ({ depends }) => {
	depends("app:machines");
	return { machines: await machinesWithStatus() };
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map