import {
  json,
} from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import {
  describeDoError,
  listDistributions,
  listDropletSnapshots,
  listRegions,
  listSizes,
  listSshKeys,
} from "#lib/doClient.js";
import { getSetting } from "#lib/server/db.js";

/**
 * Everything the machine-creation flow needs from DigitalOcean in one call:
 * available regions, size catalog (real vCPU/RAM/disk/price), droplet
 * snapshots and account SSH keys.
 */
export const GET: RequestHandler = async () => {
  try {
    const [regions, sizes, snapshots, sshKeys, distributions] = await Promise.all([
      listRegions(),
      listSizes(),
      listDropletSnapshots(),
      listSshKeys(),
      listDistributions(),
    ]);
    return json({ regions, sizes, snapshots, sshKeys, distributions, openrouter_configured: !!getSetting("openrouter_key") });
  } catch (e) {
    const d = describeDoError(e);
    return json({ error: d.message }, { status: 502 });
  }
};
