import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { SESSION_COOKIE } from "#lib/server/auth.js";

export const POST: RequestHandler = async ({ cookies }) => {
  cookies.delete(SESSION_COOKIE, { path: "/" });
  return json({ ok: true });
};
