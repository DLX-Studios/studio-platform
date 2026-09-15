// @bun
import {
  ArrowLeft,
  ArrowRight,
  Check,
  X
} from "./index-s2jwydk2.js";
import {
  attr,
  attr_class,
  bind_props,
  ensure_array_like,
  escape_html,
  stringify1 as stringify
} from "./index-e5h3mrsa.js";

// .svelte-kit/output/server/chunks/Wizard.js
function Wizard($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let { title, steps, step = 0, heading, children, error = "", nextLabel = "Continue", busyLabel = "Working\u2026", busy = false, canNext = true, onnext, onclose, variant = "page" } = $$props;
    $$renderer2.push(`<div${attr_class("card wizard", undefined, { "as-dialog": variant === "dialog" })}>`);
    if (onclose) {
      $$renderer2.push(`<!--[0--><button class="wizard-close icon-btn" aria-label="Close">`);
      X($$renderer2, { size: 14 });
      $$renderer2.push(`<!----></button>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <aside class="wizard-rail"${attr("aria-label", `${stringify(title)} steps`)}><div class="rail-title">${escape_html(title)}</div> <!--[-->`);
    const each_array = ensure_array_like(steps);
    for (let i = 0, $$length = each_array.length;i < $$length; i++) {
      let label = each_array[i];
      $$renderer2.push(`<button type="button"${attr_class("rail-step", undefined, {
        active: i === step,
        done: i < step
      })}${attr("disabled", i > step, true)}><span class="rail-index">`);
      if (i < step) {
        $$renderer2.push("<!--[0-->");
        Check($$renderer2, {
          size: 11,
          weight: "bold"
        });
      } else
        $$renderer2.push(`<!--[-1-->${escape_html(i + 1)}`);
      $$renderer2.push(`<!--]--></span> ${escape_html(label)}</button>`);
    }
    $$renderer2.push(`<!--]--></aside> <section class="wizard-body">`);
    if (error)
      $$renderer2.push(`<!--[0--><div class="alert error">${escape_html(error)}</div>`);
    else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> `);
    if (heading) {
      $$renderer2.push("<!--[0-->");
      heading($$renderer2);
      $$renderer2.push(`<!---->`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--> <div class="wizard-content">`);
    children($$renderer2);
    $$renderer2.push(`<!----></div> <div class="wizard-nav">`);
    if (step > 0) {
      $$renderer2.push(`<!--[0--><button class="btn ghost">`);
      ArrowLeft($$renderer2, { size: 16 });
      $$renderer2.push(`<!----> Back</button>`);
    } else
      $$renderer2.push(`<!--[-1--><span></span>`);
    $$renderer2.push(`<!--]--> `);
    if (onnext) {
      $$renderer2.push(`<!--[0--><button class="btn primary"${attr("disabled", busy || !canNext, true)}>${escape_html(busy ? busyLabel : nextLabel)} `);
      ArrowRight($$renderer2, { size: 16 });
      $$renderer2.push(`<!----></button>`);
    } else
      $$renderer2.push("<!--[-1-->");
    $$renderer2.push(`<!--]--></div></section></div>`);
    bind_props($$props, { step });
  });
}

export { Wizard };

//# debugId=6F5FEC216F3AEB3064756E2164756E21
