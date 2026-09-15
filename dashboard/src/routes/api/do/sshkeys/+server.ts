import { json } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { createSshKey } from "#lib/doClient.js";

/** Create an SSH key on the DO account (used by the machine wizard). */
export const POST: RequestHandler = async ({ request }) => {
  const body = (await request.json()) as { name?: string; publicKey?: string };
  const name = body.name?.trim();
  const publicKey = body.publicKey?.trim();
  if (!name || !publicKey) {
    return json({ error: "Key name and public key are required." }, { status: 400 });
  }
  if (!publicKey.startsWith("ssh-") && !publicKey.startsWith("ecdsa-") && !publicKey.startsWith("sk-")) {
    return json({ error: "That doesn't look like an OpenSSH public key (expected ssh-ed25519 …, ssh-rsa …, etc.)." }, { status: 400 });
  }
  try {
    const key = await createSshKey(name, publicKey);
    return json({ key }, { status: 201 });
  } catch (e) {
    return json({ error: (e as Error).message }, { status: 502 });
  }
};
