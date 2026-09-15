import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { analyticsSummary } from '#lib/server/analytics.js';

export const GET: RequestHandler = async () => {
  return json(analyticsSummary());
};
