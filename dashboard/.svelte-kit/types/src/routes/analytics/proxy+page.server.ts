// @ts-nocheck
import type { PageServerLoad } from './$types';
import { analyticsSummary } from '#lib/server/analytics.js';

export const load = async ({ depends }: Parameters<PageServerLoad>[0]) => {
  depends('app:analytics');
  return analyticsSummary();
};
