import { r as createSshKey } from "../../../../../../chunks/doClient.js";
import { json } from "@sveltejs/kit";
import path from "node:path";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { $ } from "bun";
//#region src/routes/api/do/sshkeys/generate/+server.ts
/**
* Generate an ed25519 keypair on the dashboard server via ssh-keygen
* (Bun shell), import the public half into DigitalOcean, and keep the
* private half on this machine only (data/ssh-keys/<name>, mode 0600 —
* ssh-keygen's default). This makes the dashboard the owner of its
* builder keypair, per Plan-01 §2.
*/
var POST = async ({ request }) => {
	const clean = (await request.json()).name?.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "-").replace(/^-+|-+$/g, "");
	if (!clean) return json({ error: "A key name is required (letters, numbers, dashes)." }, { status: 400 });
	const keysDir = path.resolve(process.cwd(), "data", "ssh-keys");
	const file = path.join(keysDir, clean);
	if (existsSync(file)) return json({ error: `A generated key named "${clean}" already exists — pick another name.` }, { status: 400 });
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
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map