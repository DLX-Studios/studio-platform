import { d as ssr_context } from "./server2.js";
//#region node_modules/svelte/src/index-server.js
/** @import { SSRContext } from '#server' */
/** @import { Renderer } from './internal/server/renderer.js' */
/** @param {() => void} fn */
function onDestroy(fn) {
	/** @type {Renderer} */ ssr_context.r.on_destroy(fn);
}
async function tick() {}
//#endregion
export { tick as n, onDestroy as t };

//# sourceMappingURL=index-server.js.map