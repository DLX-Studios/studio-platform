import { h as escape_html } from "../../../chunks/server2.js";
import { u as Sparkle } from "../../../chunks/lib.js";
import { t as AppShell } from "../../../chunks/AppShell.js";
//#region src/routes/agents/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { data } = $$props;
		AppShell($$renderer, {
			children: ($$renderer) => {
				$$renderer.push(`<main class="container"><h1>`);
				Sparkle($$renderer, {
					size: 24,
					weight: "duotone"
				});
				$$renderer.push(`<!----> AI agents</h1> <p class="hint">The coding agent provider used on your builder machines.</p> <div class="card machine-card"><div class="card-head" style="cursor: default;">`);
				Sparkle($$renderer, {
					size: 22,
					weight: "duotone"
				});
				$$renderer.push(`<!----> <strong>OpenRouter</strong></div> <p class="meta">400+ models · API key</p> <p class="meta">${escape_html(data.openrouter ? "✅ Connected — API key configured during setup." : "❌ Not configured — add a key via the setup wizard.")}</p></div> <div class="card placeholder"><p class="hint">Claude, Claude Code, Codex, OpenAI and Grok are coming — they will appear
        as selectable options in the machine wizard as they land.</p></div></main>`);
			},
			$$slots: { default: true }
		});
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map