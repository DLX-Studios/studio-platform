// @bun
import {
  ssr_context
} from "./index-e5h3mrsa.js";

// .svelte-kit/output/server/chunks/index-server.js
function onDestroy(fn) {
  ssr_context.r.on_destroy(fn);
}

export { onDestroy };

//# debugId=D6D23362D01E44BA64756E2164756E21
