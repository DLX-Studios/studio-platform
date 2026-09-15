import { getSetting } from '#lib/server/db.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
  return { doConnected: !!getSetting('do_token') };
};
