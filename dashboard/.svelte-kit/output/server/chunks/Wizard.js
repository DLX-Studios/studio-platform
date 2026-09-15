import { c as stringify, f as attr, h as escape_html, i as bind_props, o as ensure_array_like, t as attr_class } from "./server2.js";
import { B as ArrowRight, V as ArrowLeft, j as Check, t as X } from "./lib.js";
//#region src/lib/components/Wizard.svelte
function Wizard($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/** Rail heading, e.g. "Setup" or "New machine". */
		/** Step labels, rendered as the left-rail timeline. */
		/** Active step index — bindable, rail items jump back when clicked. */
		/** Fixed heading above the scroll area (icon + step title). */
		/** Scrollable step content. */
		/** Error shown pinned above the heading. */
		/** Right-hand nav button label. Omit `onnext` to hide the button. */
		/** Label while `busy`. */
		/** Renders an X close button (dialog variant). */
		/** "page" = standalone centered card; "dialog" = inside a backdrop. */
		let { title, steps, step = 0, heading, children, error = "", nextLabel = "Continue", busyLabel = "Working…", busy = false, canNext = true, onnext, onclose, variant = "page" } = $$props;
		$$renderer.push(`<div${attr_class("card wizard", void 0, { "as-dialog": variant === "dialog" })}>`);
		if (onclose) {
			$$renderer.push(`<!--[0--><button class="wizard-close icon-btn" aria-label="Close">`);
			X($$renderer, { size: 14 });
			$$renderer.push(`<!----></button>`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> <aside class="wizard-rail"${attr("aria-label", `${stringify(title)} steps`)}><div class="rail-title">${escape_html(title)}</div> <!--[-->`);
		const each_array = ensure_array_like(steps);
		for (let i = 0, $$length = each_array.length; i < $$length; i++) {
			let label = each_array[i];
			$$renderer.push(`<button type="button"${attr_class("rail-step", void 0, {
				"active": i === step,
				"done": i < step
			})}${attr("disabled", i > step, true)}><span class="rail-index">`);
			if (i < step) {
				$$renderer.push("<!--[0-->");
				Check($$renderer, {
					size: 11,
					weight: "bold"
				});
			} else $$renderer.push(`<!--[-1-->${escape_html(i + 1)}`);
			$$renderer.push(`<!--]--></span> ${escape_html(label)}</button>`);
		}
		$$renderer.push(`<!--]--></aside> <section class="wizard-body">`);
		if (error) $$renderer.push(`<!--[0--><div class="alert error">${escape_html(error)}</div>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> `);
		if (heading) {
			$$renderer.push("<!--[0-->");
			heading($$renderer);
			$$renderer.push(`<!---->`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--> <div class="wizard-content">`);
		children($$renderer);
		$$renderer.push(`<!----></div> <div class="wizard-nav">`);
		if (step > 0) {
			$$renderer.push(`<!--[0--><button class="btn ghost">`);
			ArrowLeft($$renderer, { size: 16 });
			$$renderer.push(`<!----> Back</button>`);
		} else $$renderer.push(`<!--[-1--><span></span>`);
		$$renderer.push(`<!--]--> `);
		if (onnext) {
			$$renderer.push(`<!--[0--><button class="btn primary"${attr("disabled", busy || !canNext, true)}>${escape_html(busy ? busyLabel : nextLabel)} `);
			ArrowRight($$renderer, { size: 16 });
			$$renderer.push(`<!----></button>`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></div></section></div>`);
		bind_props($$props, { step });
	});
}
//#endregion
export { Wizard as t };

//# sourceMappingURL=Wizard.js.map