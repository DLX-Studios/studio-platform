import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { verifyPin } from "#lib/server/auth.js";
import { SESSION_COOKIE, createSessionValue } from "#lib/server/auth.js";
import { addEvent } from "#lib/server/db.js";

export const POST: RequestHandler = async ({ request, cookies }) => {
  let pin: string | undefined;
  try {
    ({ pin } = (await request.json()) as { pin?: string });
  } catch {
    return json({ error: "Enter your PIN and try again." }, { status: 400 });
  }

  if (!pin) return json({ error: "PIN required" }, { status: 400 });

  try {
    if (!(await verifyPin(pin))) {
      return json({ error: "Incorrect PIN. Check it and try again." }, { status: 401 });
    }
  } catch (error) {
    console.error("PIN verification failed:", error);
    return json(
      { error: "The PIN could not be checked. The server database may be unavailable." },
      { status: 503 },
    );
  }

  cookies.set(SESSION_COOKIE, createSessionValue(), {
    path: "/",
    httpOnly: true,
    sameSite: "lax",
    maxAge: 7 * 86400,
  });
  addEvent(null, "auth", "PIN login");
  return json({ ok: true });
};
