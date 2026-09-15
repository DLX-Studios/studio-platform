/**
 * Assets bucket access — hosts the daemon binary (and future release
 * artifacts). Separate from the sccache bucket: one bucket for build
 * machines, one for distribution.
 *
 * Defaults live here; settings (assets_bucket / assets_region /
 * assets_endpoint) can override. Uses the same account-wide Spaces keys
 * as sccache.
 */
import { S3Client } from "@bradenmacdonald/s3-lite-client";
import { getSetting } from "#lib/server/db.js";

export const DAEMON_BINARY_KEY = "daemon/sb-daemon";
const DAEMON_URL_EXPIRY_SECONDS = 24 * 60 * 60; // machine boots within a day of create

export function assetsConfig() {
  return {
    bucket: getSetting("assets_bucket") ?? "studio-assets",
    region: getSetting("assets_region") ?? "atl1",
    // Region endpoint only — with virtual-host style the client prepends the
    // bucket: studio-assets.atl1.digitaloceanspaces.com
    endpoint: getSetting("assets_endpoint") ?? "atl1.digitaloceanspaces.com",
  };
}

function client() {
  const { bucket, region, endpoint } = assetsConfig();
  return new S3Client({
    endPoint: endpoint,
    region,
    bucket,
    accessKey: getSetting("spaces_key_id") ?? "",
    secretKey: getSetting("spaces_secret") ?? "",
    pathStyle: false,
  });
}

/** Short-lived signed GET URL for the daemon binary, safe to embed in user_data. */
export async function presignDaemonBinaryUrl(): Promise<string> {
  return client().getPresignedUrl("GET", DAEMON_BINARY_KEY, {
    expirySeconds: DAEMON_URL_EXPIRY_SECONDS,
  });
}
