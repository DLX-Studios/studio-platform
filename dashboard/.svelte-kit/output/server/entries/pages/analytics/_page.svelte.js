import { h as escape_html, o as ensure_array_like } from "../../../chunks/server2.js";
import { F as CaretLeft, P as ChartLine } from "../../../chunks/lib.js";
//#region src/routes/analytics/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { data } = $$props;
		$$renderer.push(`<h1>`);
		ChartLine($$renderer, { size: 28 });
		$$renderer.push(`<!----> Analytics</h1> <div class="stats"><div class="stat"><span>${escape_html(data.totalHours)}</span> <span class="stat-label">Hours Used</span></div> <div class="stat"><span>$${escape_html(data.totalCost)}</span> <span class="stat-label">Est. Cost</span></div> <div class="stat"><span>${escape_html(data.sessions.length)}</span> <span class="stat-label">Sessions</span></div></div> <div class="card" style="padding:0; overflow:hidden;"><table aria-label="Build session history"><thead><tr><th>Started</th><th>Size</th><th>Hours</th><th class="text-right">Est. Cost</th></tr></thead><tbody>`);
		const each_array = ensure_array_like(data.sessions);
		if (each_array.length !== 0) {
			$$renderer.push("<!--[-->");
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let s = each_array[$$index];
				$$renderer.push(`<tr><td>${escape_html(new Date(s.started_at).toLocaleString())}</td><td><span class="size-badge">${escape_html(s.size)}</span></td><td>${escape_html(s.hours)}</td><td class="text-right">$${escape_html(s.cost)}</td></tr>`);
			}
		} else $$renderer.push(`<!--[!--><tr><td colspan="4" style="text-align:center; opacity:0.7;">No sessions yet.</td></tr>`);
		$$renderer.push(`<!--]--></tbody></table></div> <nav><a href="/">`);
		CaretLeft($$renderer, { size: 14 });
		$$renderer.push(`<!----> Dashboard</a></nav>`);
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map