import { a as derived, f as attr, h as escape_html } from "../../../chunks/server2.js";
import "../../../chunks/navigation.js";
import { g as LockKey } from "../../../chunks/lib.js";
//#region src/routes/login/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let pin = "";
		let busy = false;
		let canSubmit = derived(() => false);
		$$renderer.push(`<main class="center-screen"><form class="card auth-card">`);
		LockKey($$renderer, {
			size: 28,
			weight: "duotone"
		});
		$$renderer.push(`<!----> <h1>Studio Builder</h1> `);
		$$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> <div class="form-group"><label for="pin">PIN</label> <input id="pin" type="password" inputmode="numeric" autocomplete="current-password"${attr("value", pin)} placeholder="••••"/></div> <button class="btn primary" type="submit"${attr("disabled", !canSubmit(), true)}${attr("aria-busy", busy)}>${escape_html("Unlock")}</button></form></main>`);
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map