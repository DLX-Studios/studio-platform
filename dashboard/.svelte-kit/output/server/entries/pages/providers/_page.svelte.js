import { h as escape_html } from "../../../chunks/server2.js";
import { E as Cloud } from "../../../chunks/lib.js";
import { t as AppShell } from "../../../chunks/AppShell.js";
//#region src/routes/providers/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { data } = $$props;
		AppShell($$renderer, {
			children: ($$renderer) => {
				$$renderer.push(`<main class="container"><h1>`);
				Cloud($$renderer, {
					size: 24,
					weight: "duotone"
				});
				$$renderer.push(`<!----> Providers</h1> <p class="hint">Cloud providers your builder machines can run on.</p> <div class="card machine-card"><div class="card-head" style="cursor: default;">`);
				Cloud($$renderer, {
					size: 22,
					weight: "duotone"
				});
				$$renderer.push(`<!----> <strong>DigitalOcean</strong></div> <p class="meta">Droplets · Spaces</p> <p class="meta">${escape_html(data.doConnected ? "✅ Connected — token configured during setup." : "❌ Not connected — run the setup wizard.")}</p></div> <div class="card placeholder"><p class="hint">AWS, Hetzner, Vultr and Google Cloud support is coming.</p></div></main>`);
			},
			$$slots: { default: true }
		});
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map