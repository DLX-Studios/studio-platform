import * as server from '../entries/pages/m/_id_/_page.server.ts.js';

export const index = 6;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/m/_id_/_page.svelte.js')).default;
export { server };
export const server_id = "src/routes/m/[id]/+page.server.ts";
export const imports = ["_app/immutable/nodes/6.D5wrDmNj.js","_app/immutable/chunks/DPBx5xTx.js","_app/immutable/chunks/D4UQy-ZY.js","_app/immutable/entry/payload.DSmR2FwN.js","_app/immutable/chunks/BIRBqmYv.js","_app/immutable/chunks/DjAkF-xo.js","_app/immutable/chunks/CV8VO5Jt.js","_app/immutable/chunks/Cene61J5.js"];
export const stylesheets = ["_app/immutable/assets/6.CAqGrTQO.css"];
export const fonts = [];
