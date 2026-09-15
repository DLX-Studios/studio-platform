#!/usr/bin/env bun
/**
 * Upload the built daemon binary to the assets bucket.
 *
 * Usage (from the dashboard/ directory, so node_modules resolves):
 *   cd dashboard && bun scripts/upload-daemon.ts [path-to-binary]
 *
 * Uploads to two keys:
 *   daemon/sb-daemon            (stable — what cloud-init downloads)
 *   daemon/sb-daemon-v<version> (versioned, for rollbacks)
 */
import { S3Client } from "@bradenmacdonald/s3-lite-client";
import { Database } from "bun:sqlite";
import path from "node:path";
import { existsSync, readFileSync } from "node:fs";

const DASHBOARD_DIR = path.resolve(import.meta.dir, "..");
const DB_PATH = path.join(DASHBOARD_DIR, "data", "studio.db");

const binaryArg = process.argv[2];
const binaryPath = binaryArg
  ? path.resolve(binaryArg)
  : path.resolve(DASHBOARD_DIR, "..", "daemon", "target", "release", "sb-daemon");

if (!existsSync(binaryPath)) {
  console.error(`Daemon binary not found at ${binaryPath}`);
  console.error("Build it first: cd daemon && cargo build --release");
  process.exit(1);
}

const db = new Database(DB_PATH, { readonly: true });
const setting = (key: string) =>
  db.query<{ value: string }, [string]>("SELECT value FROM settings WHERE key = ?").get(key)?.value;

const accessKey = setting("spaces_key_id");
const secretKey = setting("spaces_secret");
if (!accessKey || !secretKey) {
  console.error("Spaces keys are not configured (run the setup wizard first).");
  process.exit(1);
}

const bucket = setting("assets_bucket") ?? "studio-assets";
const region = setting("assets_region") ?? "atl1";
const endpoint = setting("assets_endpoint") ?? `${region}.digitaloceanspaces.com`;

// Daemon version from its Cargo.toml
const cargoToml = readFileSync(path.resolve(DASHBOARD_DIR, "..", "daemon", "Cargo.toml"), "utf8");
const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? "0.0.0";

const client = new S3Client({
  endPoint: endpoint,
  region,
  accessKey,
  secretKey,
  pathStyle: false,
});

const payload = new Uint8Array(await Bun.file(binaryPath).arrayBuffer());
console.log(`Uploading ${binaryPath} (${(payload.length / 1024 / 1024).toFixed(1)} MB) to ${bucket}.${endpoint}`);

for (const key of [`daemon/sb-daemon-v${version}`, "daemon/sb-daemon"]) {
  await client.putObject(key, payload, {
    bucketName: bucket,
    metadata: { "Content-Type": "application/octet-stream" },
  });
  console.log(`  ✓ s3://${bucket}/${key}`);
}

console.log(`Done — daemon v${version} published. Machines download the "daemon/sb-daemon" key at boot.`);
