import * as server from '../entries/pages/ssh-keys/_page.server.ts.js';

export const index = 10;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/ssh-keys/_page.svelte.js')).default;
export { server };
export const server_id = "src/routes/ssh-keys/+page.server.ts";
export const imports = ["_app/immutable/nodes/10.DcxKhWqh.js","_app/immutable/chunks/DPBx5xTx.js","_app/immutable/chunks/Cene61J5.js","_app/immutable/chunks/CmwU_1OI.js","_app/immutable/chunks/D4UQy-ZY.js","_app/immutable/entry/payload.DSmR2FwN.js","_app/immutable/chunks/BIRBqmYv.js","_app/immutable/chunks/DjAkF-xo.js","_app/immutable/chunks/CV8VO5Jt.js","_app/immutable/chunks/BTmxmbKa.js"];
export const stylesheets = ["_app/immutable/assets/OptionCard.I4OQdEuu.css"];
export const fonts = [];
