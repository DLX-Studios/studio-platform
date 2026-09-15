// @ts-nocheck
import { listSshKeys } from '#lib/doClient.js';
import type { PageServerLoad } from './$types';

export const load = async () => {
  try {
    return { keys: await listSshKeys(), error: null };
  } catch (e) {
    return { keys: [], error: (e as Error).message };
  }
};
;null as any as PageServerLoad;