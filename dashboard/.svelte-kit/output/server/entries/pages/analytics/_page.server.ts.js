import { t as analyticsSummary } from "../../../chunks/analytics.js";
//#region src/routes/analytics/+page.server.ts
var load = async ({ depends }) => {
	depends("app:analytics");
	return analyticsSummary();
};
//#endregion
export { load };

//# sourceMappingURL=_page.server.ts.js.map