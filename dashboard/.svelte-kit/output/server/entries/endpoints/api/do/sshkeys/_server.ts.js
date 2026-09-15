import { r as createSshKey } from "../../../../../chunks/doClient.js";
import { json } from "@sveltejs/kit";
//#region src/routes/api/do/sshkeys/+server.ts
/** Create an SSH key on the DO account (used by the machine wizard). */
var POST = async ({ request }) => {
	const body = await request.json();
	const name = body.name?.trim();
	const publicKey = body.publicKey?.trim();
	if (!name || !publicKey) return json({ error: "Key name and public key are required." }, { status: 400 });
	if (!publicKey.startsWith("ssh-") && !publicKey.startsWith("ecdsa-") && !publicKey.startsWith("sk-")) return json({ error: "That doesn't look like an OpenSSH public key (expected ssh-ed25519 …, ssh-rsa …, etc.)." }, { status: 400 });
	try {
		const key = await createSshKey(name, publicKey);
		return json({ key }, { status: 201 });
	} catch (e) {
		return json({ error: e.message }, { status: 502 });
	}
};
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map