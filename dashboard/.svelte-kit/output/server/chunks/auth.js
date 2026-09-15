import { c as getSetting, m as setSetting } from "./db.js";
import { createHmac, randomBytes } from "node:crypto";
//#region src/lib/server/auth.ts
var COOKIE = "sb_session";
var SESSION_DAYS = 7;
function secret() {
	let s = getSetting("session_secret");
	if (!s) {
		s = randomBytes(32).toString("hex");
		setSetting("session_secret", s);
	}
	return s;
}
async function hashPin(pin) {
	return Bun.password.hash(pin, { algorithm: "argon2id" });
}
async function verifyPin(pin) {
	const hash = getSetting("pin_hash");
	if (!hash) return false;
	return Bun.password.verify(pin, hash);
}
function sign(payload) {
	return createHmac("sha256", secret()).update(payload).digest("base64url");
}
function createSessionValue() {
	const payload = Buffer.from(JSON.stringify({ exp: Date.now() + SESSION_DAYS * 864e5 })).toString("base64url");
	return `${payload}.${sign(payload)}`;
}
function verifySessionValue(value) {
	if (!value) return false;
	const [payload, sig] = value.split(".");
	if (!payload || !sig) return false;
	const expected = sign(payload);
	if (sig.length !== expected.length) return false;
	if (!createHmac("sha256", secret()).update(payload).digest().equals(Buffer.from(sig, "base64url"))) return false;
	try {
		const { exp } = JSON.parse(Buffer.from(payload, "base64url").toString());
		return Date.now() < exp;
	} catch {
		return false;
	}
}
var SESSION_COOKIE = COOKIE;
//#endregion
export { verifySessionValue as a, verifyPin as i, createSessionValue as n, hashPin as r, SESSION_COOKIE as t };

//# sourceMappingURL=auth.js.map