// @ts-nocheck
import type { PageServerLoad } from './$types';
import { machineWithStatus } from '#lib/server/machinesStatus.js';

export const load = async ({ params, depends }: Parameters<PageServerLoad>[0]) => {
  const id = Number(params.id);
  depends(`app:machine:${id}`);
  return machineWithStatus(id);
};
