import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import template from "#lib/cloud-init-template.yml?raw";
import { machineWithStatus } from "#lib/server/machinesStatus.js";
import {
  createDroplet,
  createDropletSnapshot,
  createVolume,
  deleteDroplet,
  deleteVolume,
  detachVolume,
  listDropletsByTag,
  listVolumesByName,
  statusFromDroplet,
  sshKeyIds,
  sweepOrphanVolumes,
} from "#lib/doClient.js";
import { presignDaemonBinaryUrl } from "#lib/server/assets.js";
import { probeUntilReady } from "#lib/server/daemonProbe.js";
import {
  deleteMachine,
  endSession,
  getMachine,
  getOpenSession,
  getSetting,
  machineTag,
  setDaemonReady,
  startSession,
  updateMachine,
  addEvent,
} from "#lib/server/db.js";

/**
 * Additional storage: reuse the profile's persistent volume if it already
 * exists (it survives destroy cycles), otherwise create it. The volume is
 * attached at droplet creation so the machine always boots with it.
 */
async function ensureVolume(machine: NonNullable<ReturnType<typeof getMachine>>): Promise<number> {
  const name = `${machineTag(machine.id)}-data`;
  const existing = await listVolumesByName(name);
  const match = existing.find((v) => v.region === machine.region);
  if (match) {
    // A volume is single-attachment: detach from any stale droplet
    // (e.g. an orphan left behind by a failed create attempt) before reuse.
    for (const dropletId of match.dropletIds) {
      await detachVolume(String(match.id), dropletId).catch(() => {});
    }
    return match.id;
  }
  const created = await createVolume({ name, region: machine.region, sizeGb: machine.volume_gb });
  return created.id;
}

async function deleteVolumeByName(tag: string): Promise<void> {
  const name = `${tag}-data`;
  const vols = await listVolumesByName(name);
  for (const v of vols) {
    try {
      await deleteVolume(String(v.id));
      addEvent(null, "destroy", `Volume ${name} deleted`);
    } catch {
      // volume already gone or DO hiccup — not fatal for deletion
    }
  }
}

function buildUserData(
  machine: ReturnType<typeof getMachine>,
  daemonUrl: string,
  daemonToken: string,
  daemonBinaryUrl: string,
): string {
  let repoUrl = machine?.repo || getSetting("repo_url") || "";
  const gitToken = getSetting("git_token") ?? "";
  // Embed the token into HTTPS URLs (GitHub/GitLab PATs). SSH URLs ignore it.
  if (gitToken && repoUrl.startsWith('https://') && !repoUrl.includes("@")) {
    repoUrl = repoUrl.replace(/^https:\/\//, "https://oauth2:" + gitToken + "@");
  }
  return template
    .replace(/__REPO_URL__/g, repoUrl)
    .replace(/__DAEMON_URL__/g, daemonUrl)
    .replace(/__DAEMON_TOKEN__/g, daemonToken)
    .replace(/__DAEMON_BINARY_URL__/g, daemonBinaryUrl)
    .replace(/__SCCACHE_BUCKET__/g, getSetting("sccache_bucket") ?? "")
    .replace(/__SCCACHE_REGION__/g, getSetting("sccache_region") ?? "nyc3")
    .replace(
      /__SCCACHE_ENDPOINT__/g,
      getSetting("sccache_endpoint") ?? "nyc3.digitaloceanspaces.com",
    )
    .replace(/__AWS_ACCESS_KEY_ID__/g, getSetting("spaces_key_id") ?? "")
    .replace(/__AWS_SECRET_ACCESS_KEY__/g, getSetting("spaces_secret") ?? "")
    .replace(/__OPENROUTER_API_KEY__/g, getSetting("openrouter_key") ?? "");
}

export const GET: RequestHandler = async ({ params }) => {
  return json(await machineWithStatus(Number(params.id)));
};

export const POST: RequestHandler = async ({ params, request }) => {
  const id = Number(params.id);
  const machine = getMachine(id);
  if (!machine) return json({ error: "Not found" }, { status: 404 });

  const { action } = (await request.json()) as { action: string };
  const tag = machineTag(id);
  // Dial-home origin as reachable from this request (works on LAN and when
  // the dashboard later deploys to a public droplet).
  const daemonUrl = new URL(request.url).origin;

  if (action === "start") {
    try {
      // Idempotency: a droplet with this tag may already exist (e.g. an
      // orphan from a start attempt that failed while parsing DO's response).
      // Adopt it instead of creating a duplicate + volume conflict.
      const orphan = (await listDropletsByTag(tag)).droplets[0];
      if (orphan && (orphan.status === "new" || orphan.status === "active")) {
        setDaemonReady(id, false);
        if (!getOpenSession(id)) {
          startSession({
            machine_id: id,
            droplet_id: orphan.id,
            size: machine.size,
            region: machine.region,
            cost_per_hour: parseFloat(getSetting("cost_per_hour") ?? "0.143"),
          });
          addEvent(id, "create", `Adopted orphan droplet ${orphan.id} (recovered session)`);
        }
        return json({ ok: true, droplet_id: orphan.id, adopted: true });
      }

      // A machine boots either from a golden snapshot or a plain distribution.
      const image = machine.snapshot_id || machine.image || getSetting("snapshot_id") || "";
      if (!image) {
        return json(
          {
            error:
              "This machine has no image. Pick a distribution or golden snapshot in the machine settings (gear icon) — or build one: start the machine from a plain distribution, set it up, then take a snapshot.",
          },
          { status: 400 },
        );
      }
      const sshKeys = machine.ssh_key_ids
        ? machine.ssh_key_ids.split(",").filter(Boolean).map(Number)
        : sshKeyIds();
      if (sshKeys.length === 0) {
        return json({ error: "No SSH keys selected for this machine — add or pick one in the machine settings." }, { status: 400 });
      }
      const daemonToken = machine.daemon_token || crypto.randomUUID();
      updateMachine(id, { daemon_token: daemonToken });
      const daemonBinaryUrl = await presignDaemonBinaryUrl().catch((e) => {
        addEvent(id, "daemon", `Presign failed: ${(e as Error).message}`);
        return ""; // cloud-init skips the daemon install — machine still boots
      });
      const result = await createDroplet({
        name: machine.name,
        region: machine.region,
        size: machine.size,
        image,
        ssh_keys: sshKeys,
        tags: [tag],
        user_data: buildUserData(machine, daemonUrl, daemonToken, daemonBinaryUrl),
        dedicatedCpu: machine.dedicated === 1,
        volumes: machine.volume_gb > 0 ? [await ensureVolume(machine)] : [],
      });
      setDaemonReady(id, false);
      startSession({
        machine_id: id,
        droplet_id: result.droplet.id,
        size: machine.size,
        region: machine.region,
        cost_per_hour: parseFloat(getSetting("cost_per_hour") ?? "0.143"),
      });
      addEvent(id, "create", `Droplet ${result.droplet.id} launched`);
      return json({ ok: true, droplet_id: result.droplet.id });
    } catch (e) {
      return json({ error: (e as Error).message }, { status: 502 });
    }
  }

  if (action === "snapshot") {
    try {
      const data = await listDropletsByTag(tag);
      const droplet = data.droplets[0];
      if (!droplet) {
        return json({ error: "No running droplet to snapshot — start the machine first." }, { status: 404 });
      }
      const name = `sb-golden-${machine.name}-${new Date().toISOString().slice(0, 10)}`;
      const r = await createDropletSnapshot(droplet.id, name);
      addEvent(id, "snapshot", `Snapshot "${name}" queued (action ${r.actionId})`);
      return json({ ok: true, name, action_id: r.actionId });
    } catch (e) {
      return json({ error: (e as Error).message }, { status: 502 });
    }
  }

  if (action === "stop") {
    try {
      const data = await listDropletsByTag(tag);
      const droplet = data.droplets[0];
      if (!droplet) return json({ error: "No running droplet" }, { status: 404 });
      await deleteDroplet(droplet.id);
      endSession(droplet.id, "manual");
      setDaemonReady(id, false);
      addEvent(id, "destroy", `Droplet ${droplet.id} destroyed`);
      return json({ ok: true });
    } catch (e) {
      return json({ error: (e as Error).message }, { status: 502 });
    }
  }

  if (action === "update") {
    const body = (await request.json().catch(() => ({}))) as Record<string, unknown>;
    const updated = updateMachine(id, body as Parameters<typeof updateMachine>[1]);
    return json({ machine: updated });
  }

  if (action === "delete") {
    const data = await listDropletsByTag(tag);
    if (data.droplets[0]) {
      await deleteDroplet(data.droplets[0].id).catch(() => {});
    }
    try {
      await deleteVolumeByName(tag);
    } catch (e) {
      // Never block profile deletion — the orphan sweep will retry the volume.
      addEvent(id, "destroy", `Volume cleanup failed: ${(e as Error).message}`);
    }
    deleteMachine(id);
    addEvent(null, "machine", `Profile "${machine.name}" deleted`);
    return json({ ok: true });
  }

  return json({ error: "Unknown action" }, { status: 400 });
};
