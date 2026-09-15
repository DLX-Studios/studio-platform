import { createHmac, randomBytes } from "node:crypto";
import { getSetting, setSetting } from "./db.js";

const COOKIE = "sb_session";
const SESSION_DAYS = 7;

function secret(): string {
  let s = getSetting("session_secret");
  if (!s) {
    s = randomBytes(32).toString("hex");
    setSetting("session_secret", s);
  }
  return s;
}

export async function hashPin(pin: string): Promise<string> {
  return Bun.password.hash(pin, { algorithm: "argon2id" });
}

export async function verifyPin(pin: string): Promise<boolean> {
  const hash = getSetting("pin_hash");
  if (!hash) return false;
  return Bun.password.verify(pin, hash);
}

function sign(payload: string): string {
  return createHmac("sha256", secret()).update(payload).digest("base64url");
}

export function createSessionValue(): string {
  const payload = Buffer.from(
    JSON.stringify({ exp: Date.now() + SESSION_DAYS * 86400_000 }),
  ).toString("base64url");
  return `${payload}.${sign(payload)}`;
}

export function verifySessionValue(value: string | undefined): boolean {
  if (!value) return false;
  const [payload, sig] = value.split(".");
  if (!payload || !sig) return false;
  const expected = sign(payload);
  if (sig.length !== expected.length) return false;
  if (
    !createHmac("sha256", secret()).update(payload).digest().equals(Buffer.from(sig, "base64url"))
  ) {
    return false;
  }
  try {
    const { exp } = JSON.parse(Buffer.from(payload, "base64url").toString());
    return Date.now() < exp;
  } catch {
    return false;
  }
}

export const SESSION_COOKIE = COOKIE;
