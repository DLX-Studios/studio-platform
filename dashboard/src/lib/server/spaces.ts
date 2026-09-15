/**
 * DigitalOcean Spaces helper (S3-compatible, separate from the DO API).
 *
 * The DO Personal Access Token cannot touch Spaces — bucket operations
 * require Spaces access keys (S3 SigV4). This module uses
 * @bradenmacdonald/s3-lite-client to validate keys, locate buckets across
 * regions, and create buckets, so the wizard's "Automatic" tab only needs
 * the keypair from the user.
 *
 * Note: s3-lite-client v1 has no ListBuckets, so discovery is done per
 * bucket name via bucketExists() probed against each region endpoint.
 */
import { S3Client, S3Errors } from '@bradenmacdonald/s3-lite-client';

export const SPACES_REGIONS = [
  'nyc3',
  'sfo3',
  'ams3',
  'lon1',
  'fra1',
  'tor1',
  'syd1',
  'sgp1',
  'blr1',
] as const;

export type SpacesProbeReason = 'rejected' | 'permission' | 'network' | 'invalid';

export interface SpacesProbe {
  ok: boolean;
  exists: boolean;
  /** Region where the bucket was found/created, when known. */
  region: string | null;
  /** Regions that were probed, in order. */
  scanned: string[];
  reason: SpacesProbeReason | null;
  message: string;
}

const REASON_MESSAGES: Record<SpacesProbeReason, string> = {
  rejected: 'Spaces rejected these keys — double-check the access key and secret.',
  permission:
    'The keys were accepted but lack permission. Checking needs a Read key; creating a bucket needs “All (Buckets and Objects)”.',
  network: 'Could not reach Spaces — check your connection and try again.',
  invalid:
    'That bucket name is not valid — use 3–63 lowercase letters, numbers, dots or dashes.',
};

function client(region: string, accessKey: string, secretKey: string): S3Client {
  return new S3Client({
    endPoint: `${region}.digitaloceanspaces.com`,
    region,
    accessKey,
    secretKey,
    pathStyle: false,
  });
}

function classify(err: unknown): SpacesProbeReason {
  if (err instanceof S3Errors.InvalidBucketNameError) return 'invalid';
  if (err instanceof S3Errors.ServerError) {
    if (err.code === 'AccessDenied') return 'permission';
    // SignatureDoesNotMatch, InvalidAccessKeyId, etc.
    return 'rejected';
  }
  return 'network';
}

function fail(reason: SpacesProbeReason, scanned: string[]): SpacesProbe {
  return { ok: false, exists: false, region: null, scanned, reason, message: REASON_MESSAGES[reason] };
}

interface BucketArgs {
  accessKey: string;
  secretKey: string;
  bucket: string;
  region: string;
}

/**
 * Validate keys and find the bucket. Probes the preferred region first;
 * if the keys work but the bucket isn't there, scans the remaining
 * regions before concluding it doesn't exist anywhere.
 */
export async function checkSpacesBucket({ accessKey, secretKey, bucket, region }: BucketArgs): Promise<SpacesProbe> {
  const scanned: string[] = [];
  const order: string[] = [region, ...SPACES_REGIONS.filter((r) => r !== region)];
  let firstNetworkError: unknown = null;

  for (const r of order) {
    scanned.push(r);
    try {
      const found = await client(r, accessKey, secretKey).bucketExists(bucket);
      if (found) {
        return {
          ok: true,
          exists: true,
          region: r,
          scanned,
          reason: null,
          message: `Bucket “${bucket}” found in ${r}.`,
        };
      }
    } catch (err) {
      const reason = classify(err);
      if (reason === 'network') {
        firstNetworkError ??= err;
        continue; // a network blip in one region says nothing about the others
      }
      // Key or bucket-name problems won't improve in other regions.
      return fail(reason, scanned);
    }
  }

  if (firstNetworkError) return fail('network', scanned);
  return {
    ok: true,
    exists: false,
    region,
    scanned,
    reason: null,
    message: `Keys work. No bucket named “${bucket}” exists in any region yet — you can create it.`,
  };
}

/** Create the bucket in the given region. */
export async function createSpacesBucket({ accessKey, secretKey, bucket, region }: BucketArgs): Promise<SpacesProbe> {
  const scanned = [region];
  try {
    await client(region, accessKey, secretKey).makeBucket(bucket);
    return {
      ok: true,
      exists: true,
      region,
      scanned,
      reason: null,
      message: `Bucket “${bucket}” created in ${region}.`,
    };
  } catch (err) {
    if (err instanceof S3Errors.ServerError) {
      // Already there (e.g. created in another tab moments ago) — that's fine.
      if (err.code === 'BucketAlreadyExists' || err.code === 'BucketAlreadyOwnedByYou') {
        return {
          ok: true,
          exists: true,
          region,
          scanned,
          reason: null,
          message: `Bucket “${bucket}” already exists in ${region}.`,
        };
      }
    }
    return fail(classify(err), scanned);
  }
}
