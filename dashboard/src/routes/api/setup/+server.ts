import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { describeDoError, getAccount } from "#lib/doClient.js";
import { setSetting, addEvent } from "#lib/server/db.js";
import { hashPin, SESSION_COOKIE, createSessionValue } from "#lib/server/auth.js";

interface SetupBody {
  pin?: string;
  doToken?: string;
  sshKeyIds?: string;
  snapshotId?: string;
  repoUrl?: string;
  gitToken?: string;
  spacesBucket?: string;
  spacesRegion?: string;
  spacesEndpoint?: string;
  spacesKeyId?: string;
  spacesSecret?: string;
  openrouterKey?: string;
  region?: string;
  size?: string;
}

export const POST: RequestHandler = async ({ request, cookies }) => {
  const body = (await request.json()) as SetupBody;

  if (!body.pin || body.pin.length < 4) {
    return json({ error: "PIN must be at least 4 characters", field: "pin" }, { status: 400 });
  }
  if (!body.doToken) {
    return json(
      { error: "DigitalOcean Personal Access Token required", field: "doToken" },
      { status: 400 },
    );
  }
  if (!body.sshKeyIds) {
    return json({ error: "Select at least one SSH key", field: "sshKeyIds" }, { status: 400 });
  }
  // repoUrl is optional — it can be added later on the machine profile.
  if (!body.spacesBucket || !body.spacesKeyId || !body.spacesSecret) {
    return json(
      { error: "Spaces bucket and keys required", field: "spacesKeyId" },
      { status: 400 },
    );
  }

  try {
    await getAccount(body.doToken);
  } catch (e) {
    const d = describeDoError(e);
    return json({ error: d.message, field: "doToken", reason: d.reason }, { status: 400 });
  }

  // Token is optional — only GitHub HTTPS URLs can be validated generically.
  if (body.gitToken && body.repoUrl && body.repoUrl.startsWith('https://github.com/')) {
    try {
      const gh = await fetch("https://api.github.com/user", {
        headers: {
          Authorization: `Bearer ${body.gitToken}`,
          Accept: "application/vnd.github+json",
          "User-Agent": "studio-builder",
        },
      });
      if (gh.status === 401 || gh.status === 403) {
        return json(
          {
            error: "GitHub rejected this token (wrong or missing scopes).",
            field: "gitToken",
            reason: "rejected",
          },
          { status: 400 },
        );
      }
      if (!gh.ok) {
        return json(
          {
            error: "Could not reach GitHub. Check your connection.",
            field: "gitToken",
            reason: "network",
          },
          { status: 400 },
        );
      }
      const repoPath = body.repoUrl.replace(/^https?:\/\/github\.com\//, "").replace(/\.git$/, "");
      const repo = await fetch(`https://api.github.com/repos/${repoPath}`, {
        headers: {
          Authorization: `Bearer ${body.gitToken}`,
          Accept: "application/vnd.github+json",
          "User-Agent": "studio-builder",
        },
      });
      if (repo.status === 404) {
        return json(
          {
            error: `Repo "${repoPath}" not found, or this token cannot read it.`,
            field: "repoUrl",
            reason: "rejected",
          },
          { status: 400 },
        );
      }
    } catch {
      return json(
        {
          error: "Could not reach GitHub. Check your connection.",
          field: "gitToken",
          reason: "network",
        },
        { status: 400 },
      );
    }
  }

  if (body.openrouterKey) {
    try {
      const res = await fetch("https://openrouter.ai/api/v1/key", {
        headers: { Authorization: `Bearer ${body.openrouterKey}` },
      });
      if (!res.ok) {
        return json(
          { error: "OpenRouter rejected this key.", field: "openrouterKey", reason: "rejected" },
          { status: 400 },
        );
      }
    } catch {
      return json(
        {
          error: "Could not reach OpenRouter. Check your connection.",
          field: "openrouterKey",
          reason: "network",
        },
        { status: 400 },
      );
    }
  }

  const spacesRegion = body.spacesRegion || "nyc3";
  const spacesEndpoint = body.spacesEndpoint || `${spacesRegion}.digitaloceanspaces.com`;

  setSetting("pin_hash", await hashPin(body.pin));
  setSetting("do_token", body.doToken);
  setSetting("do_ssh_key_ids", body.sshKeyIds);
  if (body.snapshotId) setSetting("snapshot_id", body.snapshotId);
  if (body.repoUrl?.trim()) setSetting("repo_url", body.repoUrl.trim());
  if (body.gitToken) setSetting("git_token", body.gitToken);
  setSetting("sccache_bucket", body.spacesBucket);
  setSetting("sccache_region", spacesRegion);
  setSetting("sccache_endpoint", spacesEndpoint);
  setSetting("spaces_key_id", body.spacesKeyId);
  setSetting("spaces_secret", body.spacesSecret);
  if (body.openrouterKey) setSetting("openrouter_key", body.openrouterKey);
  if (body.region) setSetting("region", body.region);
  if (body.size) setSetting("size", body.size);
  setSetting("configured", "1");
  addEvent(null, "setup", "Wizard completed");

  cookies.set(SESSION_COOKIE, createSessionValue(), {
    path: "/",
    httpOnly: true,
    sameSite: "lax",
    maxAge: 7 * 86400,
  });

  return json({ ok: true });
};
