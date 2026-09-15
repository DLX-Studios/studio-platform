// @ts-nocheck
import { getSetting } from '#lib/server/db.js';
import type { PageServerLoad } from './$types';

export const load = async () => {
  return { openrouter: !!getSetting('openrouter_key') };
};
;null as any as PageServerLoad;