import { json } from "@sveltejs/kit";
import { S3Client, S3Errors } from "@bradenmacdonald/s3-lite-client";
//#region src/lib/server/spaces.ts
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
	rejected: "Spaces rejected these keys — double-check the access key and secret.",
	permission: "The keys were accepted but lack permission. Checking needs a Read key; creating a bucket needs “All (Buckets and Objects)”.",
	network: "Could not reach Spaces — check your connection and try again.",
	invalid: "That bucket name is not valid — use 3–63 lowercase letters, numbers, dots or dashes."
};
function client(region, accessKey, secretKey) {
	return new S3Client({
		endPoint: `${region}.digitaloceanspaces.com`,
		region,
		accessKey,
		secretKey,
		pathStyle: false
	});
}
function classify(err) {
	if (err instanceof S3Errors.InvalidBucketNameError) return "invalid";
	if (err instanceof S3Errors.ServerError) {
		if (err.code === "AccessDenied") return "permission";
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
/**
* Validate keys and find the bucket. Probes the preferred region first;
* if the keys work but the bucket isn't there, scans the remaining
* regions before concluding it doesn't exist anywhere.
*/
async function checkSpacesBucket({ accessKey, secretKey, bucket, region }) {
	const scanned = [];
	const order = [region, ...SPACES_REGIONS.filter((r) => r !== region)];
	let firstNetworkError = null;
	for (const r of order) {
		scanned.push(r);
		try {
			if (await client(r, accessKey, secretKey).bucketExists(bucket)) return {
				ok: true,
				exists: true,
				region: r,
				scanned,
				reason: null,
				message: `Bucket “${bucket}” found in ${r}.`
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
	if (firstNetworkError) return fail("network", scanned);
	return {
		ok: true,
		exists: false,
		region,
		scanned,
		reason: null,
		message: `Keys work. No bucket named “${bucket}” exists in any region yet — you can create it.`
	};
}
/** Create the bucket in the given region. */
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
			message: `Bucket “${bucket}” created in ${region}.`
		};
	} catch (err) {
		if (err instanceof S3Errors.ServerError) {
			if (err.code === "BucketAlreadyExists" || err.code === "BucketAlreadyOwnedByYou") return {
				ok: true,
				exists: true,
				region,
				scanned,
				reason: null,
				message: `Bucket “${bucket}” already exists in ${region}.`
			};
		}
		return fail(classify(err), scanned);
	}
}
//#endregion
//#region src/routes/api/setup/spaces/probe/+server.ts
var POST = async ({ request }) => {
	const body = await request.json();
	const accessKey = body.accessKey?.trim();
	const secretKey = body.secretKey?.trim();
	const bucket = body.bucket?.trim().toLowerCase();
	const region = SPACES_REGIONS.includes(body.region) ? body.region : "nyc3";
	if (!accessKey || !secretKey) return json({
		error: "Spaces access key and secret are required.",
		reason: "invalid"
	}, { status: 400 });
	if (!bucket) return json({
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
//#endregion
export { POST };

//# sourceMappingURL=_server.ts.js.map