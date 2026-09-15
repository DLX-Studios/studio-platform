import { f as attr, h as escape_html, o as ensure_array_like, t as attr_class } from "./server2.js";
import { t as afterNavigate } from "./navigation.js";
import { t as page } from "./state.js";
import { E as Cloud, I as CaretDown, _ as List, b as HardDrives, d as SignOut, i as UserCircle, p as Plus, u as Sparkle, y as Key } from "./lib.js";
import "./OptionCard.js";
//#region src/lib/components/AppShell.svelte
function AppShell($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { children, onnew } = $$props;
		let sidebarOpen = true;
		let newMenuOpen = false;
		let avatarMenuOpen = false;
		const NAV = [
			{
				label: "Machines",
				href: "/machines",
				icon: HardDrives
			},
			{
				label: "SSH Keys",
				href: "/ssh-keys",
				icon: Key
			},
			{
				label: "Providers",
				href: "/providers",
				icon: Cloud
			},
			{
				label: "AI Agents",
				href: "/agents",
				icon: Sparkle
			}
		];
		afterNavigate(() => {
			if (window.matchMedia("(max-width: 48rem)").matches) sidebarOpen = false;
			newMenuOpen = false;
			avatarMenuOpen = false;
		});
		$$renderer.push(`<div class="app-shell"><header class="topbar app-topbar"><div class="tb-group"><button class="icon-btn" aria-label="Toggle sidebar">`);
		List($$renderer, { size: 18 });
		$$renderer.push(`<!----></button> <a href="/machines" class="brand">`);
		Cloud($$renderer, {
			size: 20,
			weight: "duotone"
		});
		$$renderer.push(`<!----> Studio Builder</a></div> <div class="tb-group"><div class="menu-anchor-new" style="position: relative;"><button class="btn small primary" aria-haspopup="menu"${attr("aria-expanded", newMenuOpen)}>`);
		Plus($$renderer, {
			size: 14,
			weight: "bold"
		});
		$$renderer.push(`<!----> New `);
		CaretDown($$renderer, { size: 12 });
		$$renderer.push(`<!----></button> `);
		if (newMenuOpen) {
			$$renderer.push(`<!--[0--><div class="menu" role="menu"><button type="button" role="menuitem">`);
			HardDrives($$renderer, { size: 16 });
			$$renderer.push(`<!----> Machine</button> <button type="button" role="menuitem">`);
			Cloud($$renderer, { size: 16 });
			$$renderer.push(`<!----> Provider</button> <button type="button" role="menuitem">`);
			Sparkle($$renderer, { size: 16 });
			$$renderer.push(`<!----> AI Agent</button></div>`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></div> <div class="menu-anchor-avatar" style="position: relative;"><button class="avatar-btn" aria-haspopup="menu"${attr("aria-expanded", avatarMenuOpen)} aria-label="Account">`);
		UserCircle($$renderer, {
			size: 28,
			weight: "duotone"
		});
		$$renderer.push(`<!----></button> `);
		if (avatarMenuOpen) {
			$$renderer.push(`<!--[0--><div class="menu" role="menu"><button type="button" role="menuitem">`);
			SignOut($$renderer, { size: 16 });
			$$renderer.push(`<!----> Log out</button></div>`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></div></div></header> <div class="shell-body"><aside${attr_class("sidebar", void 0, { "open": sidebarOpen })}><nav aria-label="Main"><!--[-->`);
		const each_array = ensure_array_like(NAV);
		for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
			let item = each_array[$$index];
			const Icon = item.icon;
			$$renderer.push(`<a${attr("href", item.href)}${attr_class("sidebar-item", void 0, { "active": page.url.pathname === item.href })}>`);
			if (Icon) {
				$$renderer.push("<!--[-->");
				Icon($$renderer, { size: 18 });
				$$renderer.push("<!--]-->");
			} else {
				$$renderer.push("<!--[!-->");
				$$renderer.push("<!--]-->");
			}
			$$renderer.push(` ${escape_html(item.label)}</a>`);
		}
		$$renderer.push(`<!--]--></nav></aside> <main class="shell-main">`);
		children($$renderer);
		$$renderer.push(`<!----></main></div> <footer class="footer"><span>build machines, by the hour</span></footer></div> `);
		$$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> `);
		$$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]-->`);
	});
}
//#endregion
export { AppShell as t };

//# sourceMappingURL=AppShell.js.map