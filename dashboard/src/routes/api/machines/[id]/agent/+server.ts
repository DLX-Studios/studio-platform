import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { listDropletsByTag, statusFromDroplet } from "#lib/doClient.js";
import { getMachine, machineTag } from "#lib/server/db.js";

const DAEMON_PORT = 4545;

async function daemonTarget(id: number): Promise<{ url: string; token: string }> {
  const machine = getMachine(id);
  if (!machine) throw new Error("Machine not found.");
  if (!machine.daemon_token) throw new Error("This machine has not been provisioned yet.");

  const droplets = await listDropletsByTag(machineTag(id));
  const status = statusFromDroplet(droplets.droplets[0]);
  if (!status.running || !status.ip) throw new Error("The builder is not running.");
  return { url: `http://${status.ip}:${DAEMON_PORT}`, token: machine.daemon_token };
}

export const GET: RequestHandler = async ({ params, request }) => {
  try {
    const { url, token } = await daemonTarget(Number(params.id));
    const after = request.headers.get("last-event-id") ?? "0";
    const upstream = await fetch(`${url}/agent/events?after=${encodeURIComponent(after)}`, {
      headers: {
        Accept: "text/event-stream",
        Authorization: `Bearer ${token}`,
      },
      signal: request.signal,
    });
    if (!upstream.ok || !upstream.body) {
      const detail = await upstream.text().catch(() => "");
      return json(
        { error: detail || `Agent stream returned ${upstream.status}.` },
        { status: upstream.status || 502 },
      );
    }
    return new Response(upstream.body, {
      headers: {
        "Cache-Control": "no-cache, no-transform",
        Connection: "keep-alive",
        "Content-Type": "text/event-stream",
        "X-Accel-Buffering": "no",
      },
    });
  } catch (error) {
    return json({ error: (error as Error).message }, { status: 503 });
  }
};

export const POST: RequestHandler = async ({ params, request }) => {
  try {
    const command = (await request.json()) as Record<string, unknown>;
    const { url, token } = await daemonTarget(Number(params.id));
    const upstream = await fetch(`${url}/agent/command`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(command),
      signal: AbortSignal.timeout(10_000),
    });
    const body = (await upstream.json().catch(() => ({}))) as Record<string, unknown>;
    if (!upstream.ok) {
      return json(
        { error: body.error ?? `Agent command returned ${upstream.status}.` },
        { status: upstream.status },
      );
    }
    return json(body, { status: upstream.status });
  } catch (error) {
    return json({ error: (error as Error).message }, { status: 503 });
  }
};
