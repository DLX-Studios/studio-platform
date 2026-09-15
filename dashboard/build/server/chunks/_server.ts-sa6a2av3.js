// @bun
import {
  createSshKey
} from "./index-mcwdb471.js";
import"./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/do/sshkeys/_server.ts.js
var POST = async ({ request }) => {
  const body = await request.json();
  const name = body.name?.trim();
  const publicKey = body.publicKey?.trim();
  if (!name || !publicKey)
    return json({ error: "Key name and public key are required." }, { status: 400 });
  if (!publicKey.startsWith("ssh-") && !publicKey.startsWith("ecdsa-") && !publicKey.startsWith("sk-"))
    return json({ error: "That doesn't look like an OpenSSH public key (expected ssh-ed25519 \u2026, ssh-rsa \u2026, etc.)." }, { status: 400 });
  try {
    const key = await createSshKey(name, publicKey);
    return json({ key }, { status: 201 });
  } catch (e) {
    return json({ error: e.message }, { status: 502 });
  }
};
export {
  POST
};

//# debugId=22AB6AA5D5D0C4F664756E2164756E21
