import type { PageServerLoad } from './$types';
import { machinesWithStatus } from '#lib/server/machinesStatus.js';

export const load: PageServerLoad = async ({ depends }) => {
  depends('app:machines');
  return { machines: await machinesWithStatus() };
};
