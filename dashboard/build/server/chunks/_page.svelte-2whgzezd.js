// @bun
import {
  LockKey
} from "./index-s2jwydk2.js";
import"./index-6mtzd7p1.js";
import"./index-cmbc6t50.js";
import {
  attr,
  derived,
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/login/_page.svelte.js
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let pin = "";
    let busy = false;
    let canSubmit = derived(() => false);
    $$renderer2.push(`<main class="center-screen"><form class="card auth-card">`);
    LockKey($$renderer2, {
      size: 28,
      weight: "duotone"
    });
    $$renderer2.push(`<!----> <h1>Studio Builder</h1> `);
    $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <div class="form-group"><label for="pin">PIN</label> <input id="pin" type="password" inputmode="numeric" autocomplete="current-password"${attr("value", pin)} placeholder="\u2022\u2022\u2022\u2022"/></div> <button class="btn primary" type="submit"${attr("disabled", !canSubmit(), true)}${attr("aria-busy", busy)}>${escape_html("Unlock")}</button></form></main>`);
  });
}
export {
  _page as default
};

//# debugId=4244DEF5641FAFEC64756E2164756E21
