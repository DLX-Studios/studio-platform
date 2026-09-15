// @bun
import {
  attr,
  attr_class,
  escape_html,
  stringify1 as stringify
} from "./index-e5h3mrsa.js";

// .svelte-kit/output/server/chunks/OptionCard.js
function OptionCard($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { selected = false, disabled = false, compact = false, onclick, badge, children } = $$props;
    $$renderer2.push(`<button type="button"${attr_class("option-card svelte-1a44k3d", undefined, {
      selected,
      compact
    })}${attr("disabled", disabled, true)}>`);
    if (badge)
      $$renderer2.push(`<!--[0--><span${attr_class(`p-badge ${stringify(badge.kind)}`, "svelte-1a44k3d")}>${escape_html(badge.label)}</span>`);
    else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> `);
    children($$renderer2);
    $$renderer2.push(`<!----></button>`);
  });
}

export { OptionCard };

//# debugId=DC8E2735306AE6EB64756E2164756E21
