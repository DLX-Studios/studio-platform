import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { SPACES_REGIONS, checkSpacesBucket, createSpacesBucket } from '#lib/server/spaces.js';

interface SpacesProbeBody {
  accessKey?: string;
  secretKey?: string;
  region?: string;
  bucket?: string;
  action?: 'check' | 'create';
}

export const POST: RequestHandler = async ({ request }) => {
  const body = (await request.json()) as SpacesProbeBody;
  const accessKey = body.accessKey?.trim();
  const secretKey = body.secretKey?.trim();
  const bucket = body.bucket?.trim().toLowerCase();
  const region = SPACES_REGIONS.includes(body.region as never) ? body.region! : 'nyc3';

  if (!accessKey || !secretKey) {
    return json({ error: 'Spaces access key and secret are required.', reason: 'invalid' }, { status: 400 });
  }
  if (!bucket) {
    return json({ error: 'Bucket name is required.', reason: 'invalid' }, { status: 400 });
  }

  const result =
    body.action === 'create'
      ? await createSpacesBucket({ accessKey, secretKey, bucket, region })
      : await checkSpacesBucket({ accessKey, secretKey, bucket, region });

  return json(result, { status: result.ok ? 200 : 400 });
};
