import { redirect } from "@sveltejs/kit";
import type { Handle } from "@sveltejs/kit/hooks";
import { isConfigured } from "#lib/server/db.js";
import { SESSION_COOKIE, verifySessionValue } from "#lib/server/auth.js";

function isPublic(path: string): boolean {
  return (
    path === "/login" ||
    path.startsWith("/api/auth/login") ||
    path === "/setup" ||
    path.startsWith("/api/setup") ||
    // Daemon dials home with its per-machine bearer token (not cookies).
    path.startsWith("/api/daemon")
  );
}

export const handle: Handle = async ({ event, resolve }) => {
  const path = event.url.pathname;
  const isApi = path.startsWith("/api/");

  if (!isConfigured()) {
    if (!isPublic(path) && !path.startsWith("/_app")) {
      // API clients call .json() on the response — a 302 to /setup would
      // follow to HTML and surface as "Unexpected token '<', <!DOCTYPE …".
      if (isApi) {
        return Response.json({ error: "Setup not complete" }, { status: 409 });
      }
      redirect(302, "/setup");
    }
    return resolve(event);
  }

  if (path === "/setup" || path.startsWith("/api/setup")) {
    if (isApi) {
      return Response.json({ error: "Already configured" }, { status: 409 });
    }
    redirect(302, "/machines");
  }

  const authed = verifySessionValue(event.cookies.get(SESSION_COOKIE));

  if (!authed) {
    if (!isPublic(path) && !path.startsWith("/_app")) {
      // Same HTML-instead-of-JSON trap for expired sessions.
      if (isApi) {
        return Response.json({ error: "Session expired — please log in again." }, { status: 401 });
      }
      redirect(302, "/login");
    }
  } else if (path === "/login") {
    redirect(302, "/machines");
  }

  return resolve(event);
};
