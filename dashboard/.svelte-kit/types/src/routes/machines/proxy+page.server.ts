// @ts-nocheck
import type { PageServerLoad } from './$types';
import { machinesWithStatus } from '#lib/server/machinesStatus.js';

export const load = async ({ depends }: Parameters<PageServerLoad>[0]) => {
  depends('app:machines');
  return { machines: await machinesWithStatus() };
};
