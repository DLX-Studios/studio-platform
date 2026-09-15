import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { getMachineByDaemonToken, setDaemonReady, addEvent } from "#lib/server/db.js";

/**
 * Daemon dial-home endpoint. The in-guest daemon authenticates with its
 * per-machine bearer token (injected via cloud-init user_data) and reports
 * probe data. First successful hello flips the machine to "ready".
 */
interface HelloBody {
  hostname?: string;
  daemon_version?: string;
  uptime_secs?: number;
  disk_root_free_gb?: number;
  disk_workspace_free_gb?: number;
  rustc_version?: string;
  cargo_version?: string;
  sccache_version?: string;
  os?: string;
}

export const POST: RequestHandler = async ({ request }) => {
  const auth = request.headers.get("authorization") ?? "";
  const token = auth.startsWith("Bearer ") ? auth.slice(7).trim() : "";
  const machine = getMachineByDaemonToken(token);
  if (!machine) {
    return json({ ok: false, error: "Unknown daemon token" }, { status: 401 });
  }

  const body = (await request.json().catch(() => ({}))) as HelloBody;
  setDaemonReady(machine.id, true);
  addEvent(machine.id, "daemon", `Daemon v${body.daemon_version ?? "?"} online (${body.os ?? "unknown OS"})`);

  // Command queue lands in Phase 2 — empty for now.
  return json({ ok: true, machine_id: machine.id, commands: [] });
};
