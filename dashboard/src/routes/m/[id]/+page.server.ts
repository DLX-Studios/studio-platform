import type { PageServerLoad } from './$types';
import { machineWithStatus } from '#lib/server/machinesStatus.js';

export const load: PageServerLoad = async ({ params, depends }) => {
  const id = Number(params.id);
  depends(`app:machine:${id}`);
  return machineWithStatus(id);
};
