// @bun
import {
  page
} from "./index-t5a9agca.js";
import"./index-cmbc6t50.js";
import {
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/fallbacks/error.svelte.js
function Error($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    $$renderer2.push(`<h1>${escape_html(page.status)}</h1> <p>${escape_html(page.error?.message)}</p>`);
  });
}
export {
  Error as default
};

//# debugId=9F57E9420C207F2664756E2164756E21
