import { t as analyticsSummary } from "../../../../chunks/analytics.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/analytics/+server.ts
var GET = async () => {
	return json(analyticsSummary());
};
//#endregion
export { GET };

//# sourceMappingURL=_server.ts.js.map