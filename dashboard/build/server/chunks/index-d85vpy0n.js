// @bun
import {
  getSetting,
  setSetting
} from "./index-jr5xvt5y.js";

// .svelte-kit/output/server/chunks/auth.js
import { createHmac, randomBytes } from "crypto";
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
  if (!hash)
    return false;
  return Bun.password.verify(pin, hash);
}
function sign(payload) {
  return createHmac("sha256", secret()).update(payload).digest("base64url");
}
function createSessionValue() {
  const payload = Buffer.from(JSON.stringify({ exp: Date.now() + SESSION_DAYS * 86400000 })).toString("base64url");
  return `${payload}.${sign(payload)}`;
}
function verifySessionValue(value) {
  if (!value)
    return false;
  const [payload, sig] = value.split(".");
  if (!payload || !sig)
    return false;
  const expected = sign(payload);
  if (sig.length !== expected.length)
    return false;
  if (!createHmac("sha256", secret()).update(payload).digest().equals(Buffer.from(sig, "base64url")))
    return false;
  try {
    const { exp } = JSON.parse(Buffer.from(payload, "base64url").toString());
    return Date.now() < exp;
  } catch {
    return false;
  }
}
var SESSION_COOKIE = COOKIE;

export { hashPin, verifyPin, createSessionValue, verifySessionValue, SESSION_COOKIE };

//# debugId=5F0521E0CE1378C664756E2164756E21
