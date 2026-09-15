// @bun
import {
  CaretDown,
  Cloud,
  HardDrives,
  Key,
  List,
  Plus,
  SignOut,
  Sparkle,
  UserCircle
} from "./index-s2jwydk2.js";
import {
  afterNavigate
} from "./index-6mtzd7p1.js";
import {
  page
} from "./index-t5a9agca.js";
import {
  attr,
  attr_class,
  ensure_array_like,
  escape_html
} from "./index-e5h3mrsa.js";

// .svelte-kit/output/server/chunks/AppShell.js
function AppShell($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
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
      if (window.matchMedia("(max-width: 48rem)").matches)
        sidebarOpen = false;
      newMenuOpen = false;
      avatarMenuOpen = false;
    });
    $$renderer2.push(`<div class="app-shell"><header class="topbar app-topbar"><div class="tb-group"><button class="icon-btn" aria-label="Toggle sidebar">`);
    List($$renderer2, { size: 18 });
    $$renderer2.push(`<!----></button> <a href="/machines" class="brand">`);
    Cloud($$renderer2, {
      size: 20,
      weight: "duotone"
    });
    $$renderer2.push(`<!----> Studio Builder</a></div> <div class="tb-group"><div class="menu-anchor-new" style="position: relative;"><button class="btn small primary" aria-haspopup="menu"${attr("aria-expanded", newMenuOpen)}>`);
    Plus($$renderer2, {
      size: 14,
      weight: "bold"
    });
    $$renderer2.push(`<!----> New `);
    CaretDown($$renderer2, { size: 12 });
    $$renderer2.push(`<!----></button> `);
    if (newMenuOpen) {
      $$renderer2.push(`<!--[0--><div class="menu" role="menu"><button type="button" role="menuitem">`);
      HardDrives($$renderer2, { size: 16 });
      $$renderer2.push(`<!----> Machine</button> <button type="button" role="menuitem">`);
      Cloud($$renderer2, { size: 16 });
      $$renderer2.push(`<!----> Provider</button> <button type="button" role="menuitem">`);
      Sparkle($$renderer2, { size: 16 });
      $$renderer2.push(`<!----> AI Agent</button></div>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--></div> <div class="menu-anchor-avatar" style="position: relative;"><button class="avatar-btn" aria-haspopup="menu"${attr("aria-expanded", avatarMenuOpen)} aria-label="Account">`);
    UserCircle($$renderer2, {
      size: 28,
      weight: "duotone"
    });
    $$renderer2.push(`<!----></button> `);
    if (avatarMenuOpen) {
      $$renderer2.push(`<!--[0--><div class="menu" role="menu"><button type="button" role="menuitem">`);
      SignOut($$renderer2, { size: 16 });
      $$renderer2.push(`<!----> Log out</button></div>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--></div></div></header> <div class="shell-body"><aside${attr_class("sidebar", undefined, { open: sidebarOpen })}><nav aria-label="Main"><!--[-->`);
    const each_array = ensure_array_like(NAV);
    for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
      let item = each_array[$$index];
      const Icon = item.icon;
      $$renderer2.push(`<a${attr("href", item.href)}${attr_class("sidebar-item", undefined, { active: page.url.pathname === item.href })}>`);
      if (Icon) {
        $$renderer2.push("<!--[-->");
        Icon($$renderer2, { size: 18 });
        $$renderer2.push("<!--]-->");
      } else {
        $$renderer2.push("<!--[!-->");
        $$renderer2.push("<!--]-->");
      }
      $$renderer2.push(` ${escape_html(item.label)}</a>`);
    }
    $$renderer2.push(`<!--]--></nav></aside> <main class="shell-main">`);
    children($$renderer2);
    $$renderer2.push(`<!----></main></div> <footer class="footer"><span>build machines, by the hour</span></footer></div> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]-->`);
  });
}

export { AppShell };

//# debugId=AB8E0E4164254E0864756E2164756E21
