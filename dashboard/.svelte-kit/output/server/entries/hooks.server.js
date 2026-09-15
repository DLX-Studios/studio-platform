import { l as isConfigured } from "../chunks/db.js";
import { a as verifySessionValue } from "../chunks/auth.js";
import { redirect } from "@sveltejs/kit";
//#region src/hooks.server.ts
function isPublic(path) {
	return path === "/login" || path.startsWith("/api/auth/login") || path === "/setup" || path.startsWith("/api/setup") || path.startsWith("/api/daemon");
}
var handle = async ({ event, resolve }) => {
	const path = event.url.pathname;
	const isApi = path.startsWith("/api/");
	if (!isConfigured()) {
		if (!isPublic(path) && !path.startsWith("/_app")) {
			if (isApi) return Response.json({ error: "Setup not complete" }, { status: 409 });
			redirect(302, "/setup");
		}
		return resolve(event);
	}
	if (path === "/setup" || path.startsWith("/api/setup")) {
		if (isApi) return Response.json({ error: "Already configured" }, { status: 409 });
		redirect(302, "/machines");
	}
	if (!verifySessionValue(event.cookies.get("sb_session"))) {
		if (!isPublic(path) && !path.startsWith("/_app")) {
			if (isApi) return Response.json({ error: "Session expired — please log in again." }, { status: 401 });
			redirect(302, "/login");
		}
	} else if (path === "/login") redirect(302, "/machines");
	return resolve(event);
};
//#endregion
export { handle };

//# sourceMappingURL=hooks.server.js.map