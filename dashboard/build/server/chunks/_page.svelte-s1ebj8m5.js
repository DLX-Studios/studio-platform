// @bun
import {
  Wizard
} from "./index-k2y453x5.js";
import {
  OptionCard
} from "./index-h28hztms.js";
import {
  Brain,
  ChatCircleDots,
  CheckCircle,
  Cloud,
  CloudArrowUp,
  CloudFog,
  CloudLightning,
  CloudRain,
  Database,
  GitBranch,
  HardDrives,
  Key,
  Lightning,
  LockKey,
  Palette,
  Sparkle,
  Terminal,
  TerminalWindow
} from "./index-s2jwydk2.js";
import {
  goto
} from "./index-6mtzd7p1.js";
import"./index-cmbc6t50.js";
import {
  attr,
  attr_class,
  derived,
  ensure_array_like,
  escape_html
} from "./index-e5h3mrsa.js";
import"./index-qcep1gwj.js";

// .svelte-kit/output/server/entries/pages/setup/_page.svelte.js
function providerCard($$renderer, p, selectedId, onSelect, badge) {
  const Icon = p.icon;
  OptionCard($$renderer, {
    selected: p.id === selectedId,
    disabled: p.comingSoon,
    onclick: () => onSelect(p.id),
    badge,
    children: ($$renderer2) => {
      $$renderer2.push(`<span class="provider-icon">`);
      if (Icon) {
        $$renderer2.push("<!--[-->");
        Icon($$renderer2, {
          size: 22,
          weight: "duotone"
        });
        $$renderer2.push("<!--]-->");
      } else {
        $$renderer2.push("<!--[!-->");
        $$renderer2.push("<!--]-->");
      }
      $$renderer2.push(`</span> <strong>${escape_html(p.name)}</strong> <span class="meta">${escape_html(p.desc)}</span>`);
    },
    $$slots: { default: true }
  });
}
function _page($$renderer, $$props) {
  $$renderer.component(($$renderer2) => {
    let step = 0;
    let busy = false;
    let error = "";
    const STEPS = [
      "Welcome",
      "PIN",
      "Provider",
      "Repo",
      "Storage",
      "Agent",
      "Defaults",
      "Done"
    ];
    const CLOUD_PROVIDERS = [
      {
        id: "do",
        name: "DigitalOcean",
        desc: "Droplets \xB7 Spaces",
        icon: Cloud,
        comingSoon: false
      },
      {
        id: "aws",
        name: "AWS",
        desc: "EC2 \xB7 Lightsail",
        icon: CloudArrowUp,
        comingSoon: true
      },
      {
        id: "hetzner",
        name: "Hetzner",
        desc: "Cloud \xB7 Robot",
        icon: CloudLightning,
        comingSoon: true
      },
      {
        id: "vultr",
        name: "Vultr",
        desc: "Compute",
        icon: CloudRain,
        comingSoon: true
      },
      {
        id: "gcp",
        name: "Google Cloud",
        desc: "Compute Engine",
        icon: CloudFog,
        comingSoon: true
      }
    ];
    const STORAGE_PROVIDERS = [
      {
        id: "spaces",
        name: "DigitalOcean Spaces",
        desc: "S3-compatible",
        icon: HardDrives,
        comingSoon: false
      },
      {
        id: "s3",
        name: "AWS S3",
        desc: "S3-compatible",
        icon: Database,
        comingSoon: true
      },
      {
        id: "r2",
        name: "Cloudflare R2",
        desc: "S3-compatible \xB7 no egress fees",
        icon: Cloud,
        comingSoon: true
      },
      {
        id: "b2",
        name: "Backblaze B2",
        desc: "S3-compatible",
        icon: HardDrives,
        comingSoon: true
      }
    ];
    const AGENT_PROVIDERS = [
      {
        id: "openrouter",
        name: "OpenRouter",
        desc: "400+ models \xB7 API key",
        icon: Sparkle,
        comingSoon: false
      },
      {
        id: "claude",
        name: "Claude",
        desc: "Anthropic \xB7 API key",
        icon: ChatCircleDots,
        comingSoon: true
      },
      {
        id: "claude-code",
        name: "Claude Code",
        desc: "Anthropic \xB7 CLI harness",
        icon: TerminalWindow,
        comingSoon: true
      },
      {
        id: "codex",
        name: "Codex",
        desc: "OpenAI \xB7 CLI harness",
        icon: Terminal,
        comingSoon: true
      },
      {
        id: "openai",
        name: "OpenAI",
        desc: "Direct API key",
        icon: Brain,
        comingSoon: true
      },
      {
        id: "grok",
        name: "Grok",
        desc: "xAI \xB7 API key",
        icon: Lightning,
        comingSoon: true
      }
    ];
    let cloudProvider = "do";
    let storageProvider = "spaces";
    let agentProvider = "openrouter";
    let pin = "";
    let pin2 = "";
    let doToken = "";
    let selectedKeyIds = [];
    let repoUrl = "";
    let gitToken = "";
    let spacesBucket = "studio-cache";
    let spacesRegion = "nyc3";
    let spacesKeyId = "";
    let spacesSecret = "";
    let spacesCreating = false;
    let spacesProbed = false;
    let openrouterKey = "";
    let region = "nyc3";
    let size = "c-8";
    const sizes = [
      "c-4",
      "c-8",
      "c-16",
      "c-32"
    ];
    const regions = [
      "nyc3",
      "sfo3",
      "ams3",
      "lon1",
      "fra1",
      "tor1",
      "syd1",
      "sgp1",
      "blr1"
    ];
    const STEP_HEADINGS = [
      {
        label: "Welcome",
        icon: Cloud,
        weight: "duotone"
      },
      {
        label: "Choose a PIN",
        icon: LockKey,
        weight: "duotone"
      },
      {
        label: "Cloud provider",
        icon: Cloud,
        weight: "duotone"
      },
      {
        label: "Repository",
        icon: GitBranch,
        weight: "duotone"
      },
      {
        label: "Storage",
        icon: HardDrives,
        weight: "duotone"
      },
      {
        label: "AI agent",
        icon: Palette,
        weight: "duotone"
      },
      {
        label: "Defaults",
        icon: Key,
        weight: "duotone"
      },
      {
        label: "You're set",
        icon: CheckCircle,
        weight: "fill"
      }
    ];
    async function finish() {
      busy = true;
      error = "";
      try {
        const r = await fetch("/api/setup", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            pin,
            doToken,
            sshKeyIds: selectedKeyIds.join(","),
            snapshotId: undefined,
            gitToken: undefined,
            repoUrl,
            spacesBucket,
            spacesRegion,
            spacesEndpoint: `${spacesRegion}.digitaloceanspaces.com`,
            spacesKeyId,
            spacesSecret,
            openrouterKey: undefined,
            region,
            size
          })
        });
        const data = await r.json();
        if (!r.ok) {
          error = data.error;
          if (data.field === "doToken" || data.field === "sshKeyIds")
            step = 2;
          else if (data.field === "gitToken" || data.field === "repoUrl")
            step = 3;
          else if (data.field === "spacesKeyId")
            step = 4;
          else if (data.field === "openrouterKey")
            step = 5;
          else if (data.field === "pin")
            step = 1;
          busy = false;
          return;
        }
        step = 7;
      } catch (e) {
        error = e.message;
      }
      busy = false;
    }
    function next() {
      error = "";
      if (step === 1 && true) {
        error = "PIN must be at least 4 characters.";
        return;
      }
      if (step === 2 && true) {
        error = "Check the Personal Access Token first.";
        return;
      }
      if (step === 4) {
        spacesBucket.trim(), spacesKeyId.trim();
        error = "Bucket, access key, and secret are required.";
        return;
      }
      step = Math.min(step + 1, 7);
    }
    let navLabel = derived(() => step === 0 ? "Get started" : step === 6 ? "Finish setup" : step === 7 ? "Go to machines" : "Continue");
    let navCanNext = derived(() => !(step === 1 && true) && !(step === 2 && true) && !(step === 4 && (spacesBucket.trim(), !spacesKeyId.trim())));
    function navNext() {
      if (step === 6)
        return finish();
      if (step === 7)
        return goto("/machines");
      next();
    }
    function stepHeading($$renderer3, i) {
      const heading = STEP_HEADINGS[i];
      const Icon = heading.icon;
      $$renderer3.push(`<h1>`);
      if (Icon) {
        $$renderer3.push("<!--[-->");
        Icon($$renderer3, {
          size: 28,
          weight: heading.weight
        });
        $$renderer3.push("<!--]-->");
      } else {
        $$renderer3.push("<!--[!-->");
        $$renderer3.push("<!--]-->");
      }
      $$renderer3.push(` ${escape_html(heading.label)}</h1>`);
    }
    function wizardHeading($$renderer3) {
      stepHeading($$renderer3, step);
    }
    let $$settled = true;
    let $$inner_renderer;
    function $$render_inner($$renderer3) {
      $$renderer3.push(`<main class="center-screen wizard-screen">`);
      {
        let heading = function($$renderer4) {
          wizardHeading($$renderer4);
        };
        Wizard($$renderer3, {
          title: "Setup",
          steps: STEPS,
          error,
          nextLabel: navLabel(),
          busyLabel: "Validating & saving\u2026",
          busy,
          canNext: navCanNext(),
          onnext: navNext,
          get step() {
            return step;
          },
          set step($$value) {
            step = $$value;
            $$settled = false;
          },
          heading,
          children: ($$renderer4) => {
            if (step === 0)
              $$renderer4.push(`<!--[0--><p class="hint">This wizard configures Studio Builder once. Tokens stay in local SQLite
            and only leave this machine to call your cloud provider, your git host,
            object storage, AI providers, and your builders.</p>`);
            else if (step === 1)
              $$renderer4.push(`<!--[1--><div class="form-group"><label for="pin">PIN (min 4 characters)</label> <input id="pin" type="password"${attr("value", pin)}/></div> <div class="form-group"><label for="pin2">Confirm PIN</label> <input id="pin2" type="password"${attr("value", pin2)}/></div>`);
            else if (step === 2) {
              $$renderer4.push(`<!--[2--><p class="hint">Where your builder machines run. More providers are on the way.</p> <div class="option-grid"><!--[-->`);
              const each_array = ensure_array_like(CLOUD_PROVIDERS);
              for (let $$index = 0, $$length = each_array.length;$$index < $$length; $$index++) {
                let p = each_array[$$index];
                providerCard($$renderer4, p, cloudProvider, (id) => cloudProvider = id, p.comingSoon ? {
                  label: "Soon",
                  kind: "soon"
                } : undefined);
              }
              $$renderer4.push(`<!--]--></div> `);
              if (cloudProvider === "do") {
                $$renderer4.push(`<!--[0--><div class="form-group"><label for="dot">DigitalOcean Personal Access Token</label> <input id="dot" type="password"${attr("value", doToken)} placeholder="dop_v1_\u2026"/> <button type="button" class="hint-toggle">Where do I get this? \u2192</button> `);
                $$renderer4.push("<!--[-1-->");
                $$renderer4.push(`<!--]--></div> <button class="btn ghost"${attr("disabled", true, true)}>${escape_html("Check token")}</button> `);
                $$renderer4.push("<!--[-1-->");
                $$renderer4.push(`<!--]-->`);
              } else
                $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<!--]-->`);
            } else if (step === 3) {
              $$renderer4.push(`<!--[3--><p class="hint">Optional. The builder runs <code>git clone &lt;url> studio</code> at boot \u2014
            you can add or change the repository later on the machine profile.</p> <div class="form-group"><label for="rurl">Repository URL</label> <input id="rurl"${attr("value", repoUrl)} placeholder="https://github.com/owner/repo"/> <button type="button" class="hint-toggle">Which URL works? \u2192</button> `);
              $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<!--]--></div> <div class="form-group"><label for="gtok">Token (optional, for private repos)</label> <input id="gtok" type="password"${attr("value", gitToken)} placeholder="ghp_\u2026 / glpat-\u2026 (optional)"/> <button type="button" class="hint-toggle">When do I need this? \u2192</button> `);
              $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<!--]--></div>`);
            } else if (step === 4) {
              $$renderer4.push(`<!--[4--><p class="hint">Object storage for the compilation cache \u2014 it survives destroying machines.
            Region must match the bucket.</p> <div class="option-grid"><!--[-->`);
              const each_array_3 = ensure_array_like(STORAGE_PROVIDERS);
              for (let $$index_3 = 0, $$length = each_array_3.length;$$index_3 < $$length; $$index_3++) {
                let p = each_array_3[$$index_3];
                providerCard($$renderer4, p, storageProvider, (id) => storageProvider = id, p.id === "spaces" && cloudProvider === "do" ? {
                  label: "Recommended",
                  kind: "rec"
                } : p.comingSoon ? {
                  label: "Soon",
                  kind: "soon"
                } : undefined);
              }
              $$renderer4.push(`<!--]--></div> `);
              if (storageProvider === "spaces") {
                $$renderer4.push(`<!--[0--><div class="tab-row" role="tablist"><button type="button" role="tab"${attr("aria-selected", true)}${attr_class("", undefined, { active: true })}>Automatic</button> <button type="button" role="tab"${attr("aria-selected", false)}${attr_class("", undefined, { active: false })}>Manual</button></div> `);
                $$renderer4.push(`<!--[0--><p class="hint">Paste your Spaces keys and we\u2019ll find your bucket \u2014 or create one.
                Keys are generated once in the <a href="https://cloud.digitalocean.com/account/api/spaces" target="_blank" rel="noopener">Spaces key manager</a>.</p> <div class="form-row"><div class="form-group"><label for="skid">Access key</label> <input id="skid"${attr("value", spacesKeyId)}/></div> <div class="form-group"><label for="ssec">Secret key</label> <input id="ssec" type="password"${attr("value", spacesSecret)}/></div></div> <div class="form-row"><div class="form-group"><label for="sbu">Bucket name</label> <input id="sbu"${attr("value", spacesBucket)}/></div> <div class="form-group"><label for="sreg">Region</label> `);
                $$renderer4.select({
                  id: "sreg",
                  value: spacesRegion,
                  onchange: () => spacesProbed = false
                }, ($$renderer5) => {
                  $$renderer5.push(`<!--[-->`);
                  const each_array_4 = ensure_array_like(regions);
                  for (let $$index_4 = 0, $$length = each_array_4.length;$$index_4 < $$length; $$index_4++) {
                    let r = each_array_4[$$index_4];
                    $$renderer5.option({ value: r }, ($$renderer6) => {
                      $$renderer6.push(`${escape_html(r)}`);
                    });
                  }
                  $$renderer5.push(`<!--]-->`);
                });
                $$renderer4.push(`</div></div> <div class="btn-row"><button class="btn ghost"${attr("disabled", !spacesKeyId.trim(), true)}>${escape_html("Find bucket")}</button> `);
                if (spacesProbed && true)
                  $$renderer4.push(`<!--[0--><button class="btn primary"${attr("disabled", spacesCreating, true)}>${escape_html(`Create \u201C${spacesBucket}\u201D in ${spacesRegion}`)}</button>`);
                else
                  $$renderer4.push("<!--[-1-->");
                $$renderer4.push(`<!--]--></div> `);
                $$renderer4.push("<!--[-1-->");
                $$renderer4.push(`<!--]-->`);
                $$renderer4.push(`<!--]-->`);
              } else
                $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<!--]-->`);
            } else if (step === 5) {
              $$renderer4.push(`<!--[5--><p class="hint">The on-machine coding agent. Optional \u2014 you can add a provider later in Settings.</p> <div class="option-grid"><!--[-->`);
              const each_array_6 = ensure_array_like(AGENT_PROVIDERS);
              for (let $$index_6 = 0, $$length = each_array_6.length;$$index_6 < $$length; $$index_6++) {
                let p = each_array_6[$$index_6];
                providerCard($$renderer4, p, agentProvider, (id) => agentProvider = id, p.id === "openrouter" ? {
                  label: "Recommended",
                  kind: "rec"
                } : {
                  label: "Soon",
                  kind: "soon"
                });
              }
              $$renderer4.push(`<!--]--></div> `);
              if (agentProvider === "openrouter") {
                $$renderer4.push(`<!--[0--><div class="form-group"><label for="ork">OpenRouter API key</label> <input id="ork" type="password"${attr("value", openrouterKey)} placeholder="sk-or-\u2026 (optional)"/> <button type="button" class="hint-toggle">Where do I get this? \u2192</button> `);
                $$renderer4.push("<!--[-1-->");
                $$renderer4.push(`<!--]--></div>`);
              } else
                $$renderer4.push("<!--[-1-->");
              $$renderer4.push(`<!--]-->`);
            } else if (step === 6) {
              $$renderer4.push(`<!--[6--><p class="hint">Used for new machines; each profile can override.</p> <div class="form-row"><div class="form-group"><label for="reg">Region</label> `);
              $$renderer4.select({
                id: "reg",
                value: region
              }, ($$renderer5) => {
                $$renderer5.push(`<!--[-->`);
                const each_array_7 = ensure_array_like(regions);
                for (let $$index_7 = 0, $$length = each_array_7.length;$$index_7 < $$length; $$index_7++) {
                  let r = each_array_7[$$index_7];
                  $$renderer5.option({ value: r }, ($$renderer6) => {
                    $$renderer6.push(`${escape_html(r)}`);
                  });
                }
                $$renderer5.push(`<!--]-->`);
              });
              $$renderer4.push(`</div> <div class="form-group"><label for="sz">Default size</label> `);
              $$renderer4.select({
                id: "sz",
                value: size
              }, ($$renderer5) => {
                $$renderer5.push(`<!--[-->`);
                const each_array_8 = ensure_array_like(sizes);
                for (let $$index_8 = 0, $$length = each_array_8.length;$$index_8 < $$length; $$index_8++) {
                  let s = each_array_8[$$index_8];
                  $$renderer5.option({ value: s }, ($$renderer6) => {
                    $$renderer6.push(`${escape_html(s)}`);
                  });
                }
                $$renderer5.push(`<!--]-->`);
              });
              $$renderer4.push(`</div></div>`);
            } else
              $$renderer4.push(`<!--[-1--><p class="hint">Next: create a machine profile. If you skipped a snapshot, set one on the
            profile after you snapshot a prepared builder.</p>`);
            $$renderer4.push(`<!--]-->`);
          },
          $$slots: {
            heading: true,
            default: true
          }
        });
      }
      $$renderer3.push(`<!----></main>`);
    }
    do {
      $$settled = true;
      $$inner_renderer = $$renderer2.copy();
      $$render_inner($$inner_renderer);
    } while (!$$settled);
    $$renderer2.subsume($$inner_renderer);
  });
}
export {
  _page as default
};

//# debugId=F4E72EAE9B6D230064756E2164756E21
