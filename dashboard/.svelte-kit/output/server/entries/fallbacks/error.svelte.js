import { h as escape_html } from "../../chunks/server2.js";
import { t as page } from "../../chunks/state.js";
//#region node_modules/@sveltejs/kit/src/runtime/components/error.svelte
function Error($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		$$renderer.push(`<h1>${escape_html(page.status)}</h1> <p>${escape_html(page.error?.message)}</p>`);
	});
}
//#endregion
export { Error as default };

//# sourceMappingURL=error.svelte.js.map