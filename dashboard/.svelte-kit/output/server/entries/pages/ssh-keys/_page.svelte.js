import { h as escape_html, o as ensure_array_like } from "../../../chunks/server2.js";
import { y as Key } from "../../../chunks/lib.js";
import { t as AppShell } from "../../../chunks/AppShell.js";
//#region src/routes/ssh-keys/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { data } = $$props;
		AppShell($$renderer, {
			children: ($$renderer) => {
				$$renderer.push(`<main class="container"><h1>`);
				Key($$renderer, {
					size: 24,
					weight: "duotone"
				});
				$$renderer.push(`<!----> SSH keys</h1> <p class="hint">Keys on your DigitalOcean account. New machines embed the keys you select
      in the creation wizard; you can also paste or generate keys there.</p> `);
				if (data.error) $$renderer.push(`<!--[0--><div class="alert error">${escape_html(data.error)}</div>`);
				else if (data.keys.length === 0) {
					$$renderer.push(`<!--[1--><div class="card empty-state">`);
					Key($$renderer, {
						size: 36,
						weight: "duotone"
					});
					$$renderer.push(`<!----> <p>No SSH keys on this account yet.</p> <p class="hint">Create one from the New → Machine wizard's SSH keys step.</p></div>`);
				} else {
					$$renderer.push(`<!--[-1--><div class="card"><!--[-->`);
					const each_array = ensure_array_like(data.keys);
					for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
						let k = each_array[$$index];
						$$renderer.push(`<div class="key-row"><strong>${escape_html(k.name)}</strong> <span class="meta">${escape_html(k.fingerprint)}</span></div>`);
					}
					$$renderer.push(`<!--]--></div>`);
				}
				$$renderer.push(`<!--]--></main>`);
			},
			$$slots: { default: true }
		});
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map