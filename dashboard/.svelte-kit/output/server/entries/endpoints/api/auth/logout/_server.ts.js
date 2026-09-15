import { t as SESSION_COOKIE } from "../../../../../chunks/auth.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/auth/logout/+server.ts
var POST = async ({ cookies }) => {
	cookies.delete(SESSION_COOKIE, { path: "/" });
	return json({ ok: true });
};
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map