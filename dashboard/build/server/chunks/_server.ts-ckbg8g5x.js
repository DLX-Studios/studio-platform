// @bun
import {
  createSshKey
} from "./index-mcwdb471.js";
import"./index-jr5xvt5y.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/do/sshkeys/generate/_server.ts.js
import path from "path";
import { existsSync, mkdirSync, readFileSync } from "fs";
var {$ } = globalThis.Bun;
var POST = async ({ request }) => {
  const clean = (await request.json()).name?.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "-").replace(/^-+|-+$/g, "");
  if (!clean)
    return json({ error: "A key name is required (letters, numbers, dashes)." }, { status: 400 });
  const keysDir = path.resolve(process.cwd(), "data", "ssh-keys");
  const file = path.join(keysDir, clean);
  if (existsSync(file))
    return json({ error: `A generated key named "${clean}" already exists \u2014 pick another name.` }, { status: 400 });
  mkdirSync(keysDir, { recursive: true });
  try {
    await $`ssh-keygen -t ed25519 -f ${file} -N "" -C ${`studio-builder-${clean}`}`.quiet();
  } catch (e) {
    return json({ error: `ssh-keygen failed: ${e.message}. Is OpenSSH installed on the dashboard host?` }, { status: 500 });
  }
  let publicKey;
  try {
    publicKey = readFileSync(`${file}.pub`, "utf8").trim();
  } catch {
    return json({ error: "Key was generated but the public half could not be read." }, { status: 500 });
  }
  try {
    const key = await createSshKey(`studio-builder-${clean}`, publicKey);
    return json({
      key,
      privateKeyPath: file
    }, { status: 201 });
  } catch (e) {
    try {
      await $`rm -f ${file} ${file}.pub`.quiet();
    } catch {}
    return json({ error: `DigitalOcean rejected the key: ${e.message}` }, { status: 502 });
  }
};
export {
  POST
};

//# debugId=01296D3505AAD02564756E2164756E21
