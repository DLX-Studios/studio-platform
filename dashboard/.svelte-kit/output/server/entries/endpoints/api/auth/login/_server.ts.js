import { t as addEvent } from "../../../../../chunks/db.js";
import { i as verifyPin, n as createSessionValue, t as SESSION_COOKIE } from "../../../../../chunks/auth.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/auth/login/+server.ts
var POST = async ({ request, cookies }) => {
	let pin;
	try {
		({pin} = await request.json());
	} catch {
		return json({ error: "Enter your PIN and try again." }, { status: 400 });
	}
	if (!pin) return json({ error: "PIN required" }, { status: 400 });
	try {
		if (!await verifyPin(pin)) return json({ error: "Incorrect PIN. Check it and try again." }, { status: 401 });
	} catch (error) {
		console.error("PIN verification failed:", error);
		return json({ error: "The PIN could not be checked. The server database may be unavailable." }, { status: 503 });
	}
	cookies.set(SESSION_COOKIE, createSessionValue(), {
		path: "/",
		httpOnly: true,
		sameSite: "lax",
		maxAge: 604800
	});
	addEvent(null, "auth", "PIN login");
	return json({ ok: true });
};
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map