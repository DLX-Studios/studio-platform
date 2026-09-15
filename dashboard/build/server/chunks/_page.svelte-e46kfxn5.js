// @bun
import {
  AppShell
} from "./index-m8cbja16.js";
import"./index-h28hztms.js";
import {
  Key
} from "./index-s2jwydk2.js";
import"./index-6mtzd7p1.js";
import"./index-t5a9agca.js";
import"./index-cmbc6t50.js";
import {
  ensure_array_like,
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/ssh-keys/_page.svelte.js
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { data } = $$props;
    AppShell($$renderer2, {
      children: ($$renderer3) => {
        $$renderer3.push(`<main class="container"><h1>`);
        Key($$renderer3, {
          size: 24,
          weight: "duotone"
        });
        $$renderer3.push(`<!----> SSH keys</h1> <p class="hint">Keys on your DigitalOcean account. New machines embed the keys you select
      in the creation wizard; you can also paste or generate keys there.</p> `);
        if (data.error)
          $$renderer3.push(`<!--[0--><div class="alert error">${escape_html(data.error)}</div>`);
        else if (data.keys.length === 0) {
          $$renderer3.push(`<!--[1--><div class="card empty-state">`);
          Key($$renderer3, {
            size: 36,
            weight: "duotone"
          });
          $$renderer3.push(`<!----> <p>No SSH keys on this account yet.</p> <p class="hint">Create one from the New \u2192 Machine wizard's SSH keys step.</p></div>`);
        } else {
          $$renderer3.push(`<!--[-1--><div class="card"><!--[-->`);
          const each_array = ensure_array_like(data.keys);
          for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
            let k = each_array[$$index];
            $$renderer3.push(`<div class="key-row"><strong>${escape_html(k.name)}</strong> <span class="meta">${escape_html(k.fingerprint)}</span></div>`);
          }
          $$renderer3.push(`<!--]--></div>`);
        }
        $$renderer3.push(`<!--]--></main>`);
      },
      $$slots: { default: true }
    });
  });
}
export {
  _page as default
};

//# debugId=5CA9E74EA678EA4664756E2164756E21
