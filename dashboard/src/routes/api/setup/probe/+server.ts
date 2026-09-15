import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { describeDoError, getAccount, listDropletSnapshots, listSshKeys } from "#lib/doClient.js";

interface ProbeBody {
  token?: string;
}

export const POST: RequestHandler = async ({ request }) => {
  const { token } = (await request.json()) as ProbeBody;
  if (!token) return json({ error: "Token required", reason: "rejected" }, { status: 400 });

  try {
    await getAccount(token);
  } catch (e) {
    const d = describeDoError(e);
    return json({ error: d.message, reason: d.reason }, { status: 400 });
  }

  const keys = await listSshKeys(token).catch(() => []);
  const snapshots = await listDropletSnapshots(token).catch(() => []);

  return json({ ok: true, keys, snapshots });
};
