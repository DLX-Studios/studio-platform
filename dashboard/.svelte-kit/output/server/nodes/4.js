import * as server from '../entries/pages/analytics/_page.server.ts.js';

export const index = 4;
let component_cache;
export const component = async () => component_cache ??= (await import('../entries/pages/analytics/_page.svelte.js')).default;
export { server };
export const server_id = "src/routes/analytics/+page.server.ts";
export const imports = ["_app/immutable/nodes/4.DgDOe5kj.js","_app/immutable/chunks/DPBx5xTx.js","_app/immutable/chunks/Cene61J5.js"];
export const stylesheets = [];
export const fonts = [];
