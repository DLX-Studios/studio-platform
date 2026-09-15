// @bun
import {
  AppShell
} from "./index-m8cbja16.js";
import"./index-h28hztms.js";
import {
  Sparkle
} from "./index-s2jwydk2.js";
import"./index-6mtzd7p1.js";
import"./index-t5a9agca.js";
import"./index-cmbc6t50.js";
import {
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/agents/_page.svelte.js
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { data } = $$props;
    AppShell($$renderer2, {
      children: ($$renderer3) => {
        $$renderer3.push(`<main class="container"><h1>`);
        Sparkle($$renderer3, {
          size: 24,
          weight: "duotone"
        });
        $$renderer3.push(`<!----> AI agents</h1> <p class="hint">The coding agent provider used on your builder machines.</p> <div class="card machine-card"><div class="card-head" style="cursor: default;">`);
        Sparkle($$renderer3, {
          size: 22,
          weight: "duotone"
        });
        $$renderer3.push(`<!----> <strong>OpenRouter</strong></div> <p class="meta">400+ models \xB7 API key</p> <p class="meta">${escape_html(data.openrouter ? "\u2705 Connected \u2014 API key configured during setup." : "\u274C Not configured \u2014 add a key via the setup wizard.")}</p></div> <div class="card placeholder"><p class="hint">Claude, Claude Code, Codex, OpenAI and Grok are coming \u2014 they will appear
        as selectable options in the machine wizard as they land.</p></div></main>`);
      },
      $$slots: { default: true }
    });
  });
}
export {
  _page as default
};

//# debugId=472ACEEAC6EFE86F64756E2164756E21
