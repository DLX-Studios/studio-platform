import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { listAllDroplets, statusFromDroplet, sweepOrphanVolumes } from "#lib/doClient.js";
import { machinesWithStatus } from "#lib/server/machinesStatus.js";
import { createMachine, getSetting, machineTag, addEvent } from "#lib/server/db.js";
import { probeUntilReady } from "#lib/server/daemonProbe.js";

interface CreateBody {
  name?: string;
  size?: string;
  region?: string;
  snapshotId?: string;
  imageSlug?: string;
  repo?: string;
  idleTimeoutMin?: number;
  dedicated?: boolean;
  volumeGb?: number;
  sshKeyIds?: string;
}

export const GET: RequestHandler = async () => {
  return json({ machines: await machinesWithStatus() });
};

export const POST: RequestHandler = async ({ request }) => {
  const body = (await request.json()) as CreateBody;
  if (!body.name?.trim()) return json({ error: "Name required" }, { status: 400 });

  const machine = createMachine({
    name: body.name.trim(),
    size: body.size || getSetting("size") || "c-8",
    region: body.region || getSetting("region") || "nyc3",
    snapshot_id: body.snapshotId || "",
    image: body.imageSlug || "",
    repo: body.repo || null,
    idle_timeout_min: body.idleTimeoutMin ?? 0,
    dedicated: body.dedicated ?? false,
    volume_gb: body.volumeGb ?? 0,
    ssh_key_ids: body.sshKeyIds ?? "",
  });
  addEvent(machine.id, "machine", `Profile "${machine.name}" created`);
  return json({ machine }, { status: 201 });
};
