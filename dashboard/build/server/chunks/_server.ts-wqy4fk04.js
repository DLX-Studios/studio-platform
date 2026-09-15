// @bun
import {
  SESSION_COOKIE,
  createSessionValue,
  verifyPin
} from "./index-d85vpy0n.js";
import {
  addEvent
} from "./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/auth/login/_server.ts.js
var POST = async ({ request, cookies }) => {
  let pin;
  try {
    ({ pin } = await request.json());
  } catch {
    return json({ error: "Enter your PIN and try again." }, { status: 400 });
  }
  if (!pin)
    return json({ error: "PIN required" }, { status: 400 });
  try {
    if (!await verifyPin(pin))
      return json({ error: "Incorrect PIN. Check it and try again." }, { status: 401 });
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
export {
  POST
};

//# debugId=0429279916DEF5C364756E2164756E21
