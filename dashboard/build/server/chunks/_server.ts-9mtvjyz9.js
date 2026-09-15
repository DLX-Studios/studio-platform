// @bun
import {
  Client,
  exports_errors
} from "./index-z8d1ece1.js";
import {
  json
} from "./index-pcpfewry.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/endpoints/api/setup/spaces/probe/_server.ts.js
var SPACES_REGIONS = [
  "nyc3",
  "sfo3",
  "ams3",
  "lon1",
  "fra1",
  "tor1",
  "syd1",
  "sgp1",
  "blr1"
];
var REASON_MESSAGES = {
  rejected: "Spaces rejected these keys \u2014 double-check the access key and secret.",
  permission: "The keys were accepted but lack permission. Checking needs a Read key; creating a bucket needs \u201CAll (Buckets and Objects)\u201D.",
  network: "Could not reach Spaces \u2014 check your connection and try again.",
  invalid: "That bucket name is not valid \u2014 use 3\u201363 lowercase letters, numbers, dots or dashes."
};
function client(region, accessKey, secretKey) {
  return new Client({
    endPoint: `${region}.digitaloceanspaces.com`,
    region,
    accessKey,
    secretKey,
    pathStyle: false
  });
}
function classify(err) {
  if (err instanceof exports_errors.InvalidBucketNameError)
    return "invalid";
  if (err instanceof exports_errors.ServerError) {
    if (err.code === "AccessDenied")
      return "permission";
    return "rejected";
  }
  return "network";
}
function fail(reason, scanned) {
  return {
    ok: false,
    exists: false,
    region: null,
    scanned,
    reason,
    message: REASON_MESSAGES[reason]
  };
}
async function checkSpacesBucket({ accessKey, secretKey, bucket, region }) {
  const scanned = [];
  const order = [region, ...SPACES_REGIONS.filter((r) => r !== region)];
  let firstNetworkError = null;
  for (const r of order) {
    scanned.push(r);
    try {
      if (await client(r, accessKey, secretKey).bucketExists(bucket))
        return {
          ok: true,
          exists: true,
          region: r,
          scanned,
          reason: null,
          message: `Bucket \u201C${bucket}\u201D found in ${r}.`
        };
    } catch (err) {
      const reason = classify(err);
      if (reason === "network") {
        firstNetworkError ??= err;
        continue;
      }
      return fail(reason, scanned);
    }
  }
  if (firstNetworkError)
    return fail("network", scanned);
  return {
    ok: true,
    exists: false,
    region,
    scanned,
    reason: null,
    message: `Keys work. No bucket named \u201C${bucket}\u201D exists in any region yet \u2014 you can create it.`
  };
}
async function createSpacesBucket({ accessKey, secretKey, bucket, region }) {
  const scanned = [region];
  try {
    await client(region, accessKey, secretKey).makeBucket(bucket);
    return {
      ok: true,
      exists: true,
      region,
      scanned,
      reason: null,
      message: `Bucket \u201C${bucket}\u201D created in ${region}.`
    };
  } catch (err) {
    if (err instanceof exports_errors.ServerError) {
      if (err.code === "BucketAlreadyExists" || err.code === "BucketAlreadyOwnedByYou")
        return {
          ok: true,
          exists: true,
          region,
          scanned,
          reason: null,
          message: `Bucket \u201C${bucket}\u201D already exists in ${region}.`
        };
    }
    return fail(classify(err), scanned);
  }
}
var POST = async ({ request }) => {
  const body = await request.json();
  const accessKey = body.accessKey?.trim();
  const secretKey = body.secretKey?.trim();
  const bucket = body.bucket?.trim().toLowerCase();
  const region = SPACES_REGIONS.includes(body.region) ? body.region : "nyc3";
  if (!accessKey || !secretKey)
    return json({
      error: "Spaces access key and secret are required.",
      reason: "invalid"
    }, { status: 400 });
  if (!bucket)
    return json({
      error: "Bucket name is required.",
      reason: "invalid"
    }, { status: 400 });
  const result = body.action === "create" ? await createSpacesBucket({
    accessKey,
    secretKey,
    bucket,
    region
  }) : await checkSpacesBucket({
    accessKey,
    secretKey,
    bucket,
    region
  });
  return json(result, { status: result.ok ? 200 : 400 });
};
export {
  POST
};

//# debugId=FC7937208C3BC1C664756E2164756E21
