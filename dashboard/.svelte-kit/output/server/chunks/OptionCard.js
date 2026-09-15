import { c as stringify, f as attr, h as escape_html, t as attr_class } from "./server2.js";
//#region src/lib/components/OptionCard.svelte
function OptionCard($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { selected = false, disabled = false, compact = false, onclick, badge, children } = $$props;
		$$renderer.push(`<button type="button"${attr_class("option-card svelte-1a44k3d", void 0, {
			"selected": selected,
			"compact": compact
		})}${attr("disabled", disabled, true)}>`);
		if (badge) $$renderer.push(`<!--[0--><span${attr_class(`p-badge ${stringify(badge.kind)}`, "svelte-1a44k3d")}>${escape_html(badge.label)}</span>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> `);
		children($$renderer);
		$$renderer.push(`<!----></button>`);
	});
}
//#endregion
export { OptionCard as t };

//# sourceMappingURL=OptionCard.js.map