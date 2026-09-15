// @bun
import {
  AppShell
} from "./index-m8cbja16.js";
import"./index-h28hztms.js";
import {
  Cloud
} from "./index-s2jwydk2.js";
import"./index-6mtzd7p1.js";
import"./index-t5a9agca.js";
import"./index-cmbc6t50.js";
import {
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/providers/_page.svelte.js
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { data } = $$props;
    AppShell($$renderer2, {
      children: ($$renderer3) => {
        $$renderer3.push(`<main class="container"><h1>`);
        Cloud($$renderer3, {
          size: 24,
          weight: "duotone"
        });
        $$renderer3.push(`<!----> Providers</h1> <p class="hint">Cloud providers your builder machines can run on.</p> <div class="card machine-card"><div class="card-head" style="cursor: default;">`);
        Cloud($$renderer3, {
          size: 22,
          weight: "duotone"
        });
        $$renderer3.push(`<!----> <strong>DigitalOcean</strong></div> <p class="meta">Droplets \xB7 Spaces</p> <p class="meta">${escape_html(data.doConnected ? "\u2705 Connected \u2014 token configured during setup." : "\u274C Not connected \u2014 run the setup wizard.")}</p></div> <div class="card placeholder"><p class="hint">AWS, Hetzner, Vultr and Google Cloud support is coming.</p></div></main>`);
      },
      $$slots: { default: true }
    });
  });
}
export {
  _page as default
};

//# debugId=7B2D28F72402474364756E2164756E21
