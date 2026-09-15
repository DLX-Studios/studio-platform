import { t as machineWithStatus } from "../../../../chunks/machinesStatus.js";
//#region src/routes/m/[id]/+page.server.ts
var load = async ({ params, depends }) => {
	const id = Number(params.id);
	depends(`app:machine:${id}`);
	return machineWithStatus(id);
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map