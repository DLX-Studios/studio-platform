import type { PageServerLoad } from './$types';
import { analyticsSummary } from '#lib/server/analytics.js';

export const load: PageServerLoad = async ({ depends }) => {
  depends('app:analytics');
  return analyticsSummary();
};
