// @ts-nocheck
import { getSetting } from '#lib/server/db.js';
import type { PageServerLoad } from './$types';

export const load = async () => {
  return { doConnected: !!getSetting('do_token') };
};
;null as any as PageServerLoad;