// @bun
import {
  CaretLeft,
  ChartLine
} from "./index-s2jwydk2.js";
import"./index-cmbc6t50.js";
import {
  ensure_array_like,
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/analytics/_page.svelte.js
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { data } = $$props;
    $$renderer2.push(`<h1>`);
    ChartLine($$renderer2, { size: 28 });
    $$renderer2.push(`<!----> Analytics</h1> <div class="stats"><div class="stat"><span>${escape_html(data.totalHours)}</span> <span class="stat-label">Hours Used</span></div> <div class="stat"><span>$${escape_html(data.totalCost)}</span> <span class="stat-label">Est. Cost</span></div> <div class="stat"><span>${escape_html(data.sessions.length)}</span> <span class="stat-label">Sessions</span></div></div> <div class="card" style="padding:0; overflow:hidden;"><table aria-label="Build session history"><thead><tr><th>Started</th><th>Size</th><th>Hours</th><th class="text-right">Est. Cost</th></tr></thead><tbody>`);
    const each_array = ensure_array_like(data.sessions);
    if (each_array.length !== 0) {
      $$renderer2.push("<!--[-->");
      for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
        let s = each_array[$$index];
        $$renderer2.push(`<tr><td>${escape_html(new Date(s.started_at).toLocaleString())}</td><td><span class="size-badge">${escape_html(s.size)}</span></td><td>${escape_html(s.hours)}</td><td class="text-right">$${escape_html(s.cost)}</td></tr>`);
      }
    } else
      $$renderer2.push(`<!--[!--><tr><td colspan="4" style="text-align:center; opacity:0.7;">No sessions yet.</td></tr>`);
    $$renderer2.push(`<!--]--></tbody></table></div> <nav><a href="/">`);
    CaretLeft($$renderer2, { size: 14 });
    $$renderer2.push(`<!----> Dashboard</a></nav>`);
  });
}
export {
  _page as default
};

//# debugId=CDAFA9950BEEF06B64756E2164756E21
