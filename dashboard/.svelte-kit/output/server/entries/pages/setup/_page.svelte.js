import { a as derived, f as attr, h as escape_html, o as ensure_array_like, t as attr_class } from "../../../chunks/server2.js";
import { n as goto } from "../../../chunks/navigation.js";
import { A as CloudArrowUp, D as CloudRain, E as Cloud, M as CheckCircle, N as ChatCircleDots, O as CloudLightning, R as Brain, b as HardDrives, g as LockKey, h as Palette, k as CloudFog, o as Terminal, s as TerminalWindow, u as Sparkle, v as Lightning, w as Database, x as GitBranch, y as Key } from "../../../chunks/lib.js";
import { t as OptionCard } from "../../../chunks/OptionCard.js";
import { t as Wizard } from "../../../chunks/Wizard.js";
//#region src/routes/setup/+page.svelte
function providerCard($$renderer, p, selectedId, onSelect, badge) {
	const Icon = p.icon;
	OptionCard($$renderer, {
		selected: p.id === selectedId,
		disabled: p.comingSoon,
		onclick: () => onSelect(p.id),
		badge,
		children: ($$renderer) => {
			$$renderer.push(`<span class="provider-icon">`);
			if (Icon) {
				$$renderer.push("<!--[-->");
				Icon($$renderer, {
					size: 22,
					weight: "duotone"
				});
				$$renderer.push("<!--]-->");
			} else {
				$$renderer.push("<!--[!-->");
				$$renderer.push("<!--]-->");
			}
			$$renderer.push(`</span> <strong>${escape_html(p.name)}</strong> <span class="meta">${escape_html(p.desc)}</span>`);
		},
		$$slots: { default: true }
	});
}
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
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
				desc: "Droplets · Spaces",
				icon: Cloud,
				comingSoon: false
			},
			{
				id: "aws",
				name: "AWS",
				desc: "EC2 · Lightsail",
				icon: CloudArrowUp,
				comingSoon: true
			},
			{
				id: "hetzner",
				name: "Hetzner",
				desc: "Cloud · Robot",
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
				desc: "S3-compatible · no egress fees",
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
				desc: "400+ models · API key",
				icon: Sparkle,
				comingSoon: false
			},
			{
				id: "claude",
				name: "Claude",
				desc: "Anthropic · API key",
				icon: ChatCircleDots,
				comingSoon: true
			},
			{
				id: "claude-code",
				name: "Claude Code",
				desc: "Anthropic · CLI harness",
				icon: TerminalWindow,
				comingSoon: true
			},
			{
				id: "codex",
				name: "Codex",
				desc: "OpenAI · CLI harness",
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
				desc: "xAI · API key",
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
						snapshotId: void 0,
						gitToken: void 0,
						repoUrl,
						spacesBucket,
						spacesRegion,
						spacesEndpoint: `${spacesRegion}.digitaloceanspaces.com`,
						spacesKeyId,
						spacesSecret,
						openrouterKey: void 0,
						region,
						size
					})
				});
				const data = await r.json();
				if (!r.ok) {
					error = data.error;
					if (data.field === "doToken" || data.field === "sshKeyIds") step = 2;
					else if (data.field === "gitToken" || data.field === "repoUrl") step = 3;
					else if (data.field === "spacesKeyId") step = 4;
					else if (data.field === "openrouterKey") step = 5;
					else if (data.field === "pin") step = 1;
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
			if (step === 6) return finish();
			if (step === 7) return goto("/machines");
			next();
		}
		function stepHeading($$renderer, i) {
			const heading = STEP_HEADINGS[i];
			const Icon = heading.icon;
			$$renderer.push(`<h1>`);
			if (Icon) {
				$$renderer.push("<!--[-->");
				Icon($$renderer, {
					size: 28,
					weight: heading.weight
				});
				$$renderer.push("<!--]-->");
			} else {
				$$renderer.push("<!--[!-->");
				$$renderer.push("<!--]-->");
			}
			$$renderer.push(` ${escape_html(heading.label)}</h1>`);
		}
		function wizardHeading($$renderer) {
			stepHeading($$renderer, step);
		}
		let $$settled = true;
		let $$inner_renderer;
		function $$render_inner($$renderer) {
			$$renderer.push(`<main class="center-screen wizard-screen">`);
			{
				function heading($$renderer) {
					wizardHeading($$renderer);
				}
				Wizard($$renderer, {
					title: "Setup",
					steps: STEPS,
					error,
					nextLabel: navLabel(),
					busyLabel: "Validating & saving…",
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
					children: ($$renderer) => {
						if (step === 0) $$renderer.push(`<!--[0--><p class="hint">This wizard configures Studio Builder once. Tokens stay in local SQLite
            and only leave this machine to call your cloud provider, your git host,
            object storage, AI providers, and your builders.</p>`);
						else if (step === 1) $$renderer.push(`<!--[1--><div class="form-group"><label for="pin">PIN (min 4 characters)</label> <input id="pin" type="password"${attr("value", pin)}/></div> <div class="form-group"><label for="pin2">Confirm PIN</label> <input id="pin2" type="password"${attr("value", pin2)}/></div>`);
						else if (step === 2) {
							$$renderer.push(`<!--[2--><p class="hint">Where your builder machines run. More providers are on the way.</p> <div class="option-grid"><!--[-->`);
							const each_array = ensure_array_like(CLOUD_PROVIDERS);
							for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
								let p = each_array[$$index];
								providerCard($$renderer, p, cloudProvider, (id) => cloudProvider = id, p.comingSoon ? {
									label: "Soon",
									kind: "soon"
								} : void 0);
							}
							$$renderer.push(`<!--]--></div> `);
							if (cloudProvider === "do") {
								$$renderer.push(`<!--[0--><div class="form-group"><label for="dot">DigitalOcean Personal Access Token</label> <input id="dot" type="password"${attr("value", doToken)} placeholder="dop_v1_…"/> <button type="button" class="hint-toggle">Where do I get this? →</button> `);
								$$renderer.push("<!--[-1-->");
								$$renderer.push(`<!--]--></div> <button class="btn ghost"${attr("disabled", true, true)}>${escape_html("Check token")}</button> `);
								$$renderer.push("<!--[-1-->");
								$$renderer.push(`<!--]-->`);
							} else $$renderer.push("<!--[-1-->");
							$$renderer.push(`<!--]-->`);
						} else if (step === 3) {
							$$renderer.push(`<!--[3--><p class="hint">Optional. The builder runs <code>git clone &lt;url> studio</code> at boot —
            you can add or change the repository later on the machine profile.</p> <div class="form-group"><label for="rurl">Repository URL</label> <input id="rurl"${attr("value", repoUrl)} placeholder="https://github.com/owner/repo"/> <button type="button" class="hint-toggle">Which URL works? →</button> `);
							$$renderer.push("<!--[-1-->");
							$$renderer.push(`<!--]--></div> <div class="form-group"><label for="gtok">Token (optional, for private repos)</label> <input id="gtok" type="password"${attr("value", gitToken)} placeholder="ghp_… / glpat-… (optional)"/> <button type="button" class="hint-toggle">When do I need this? →</button> `);
							$$renderer.push("<!--[-1-->");
							$$renderer.push(`<!--]--></div>`);
						} else if (step === 4) {
							$$renderer.push(`<!--[4--><p class="hint">Object storage for the compilation cache — it survives destroying machines.
            Region must match the bucket.</p> <div class="option-grid"><!--[-->`);
							const each_array_3 = ensure_array_like(STORAGE_PROVIDERS);
							for (let $$index_3 = 0, $$length = each_array_3.length; $$index_3 < $$length; $$index_3++) {
								let p = each_array_3[$$index_3];
								providerCard($$renderer, p, storageProvider, (id) => storageProvider = id, p.id === "spaces" && cloudProvider === "do" ? {
									label: "Recommended",
									kind: "rec"
								} : p.comingSoon ? {
									label: "Soon",
									kind: "soon"
								} : void 0);
							}
							$$renderer.push(`<!--]--></div> `);
							if (storageProvider === "spaces") {
								$$renderer.push(`<!--[0--><div class="tab-row" role="tablist"><button type="button" role="tab"${attr("aria-selected", true)}${attr_class("", void 0, { "active": true })}>Automatic</button> <button type="button" role="tab"${attr("aria-selected", false)}${attr_class("", void 0, { "active": false })}>Manual</button></div> `);
								$$renderer.push(`<!--[0--><p class="hint">Paste your Spaces keys and we’ll find your bucket — or create one.
                Keys are generated once in the <a href="https://cloud.digitalocean.com/account/api/spaces" target="_blank" rel="noopener">Spaces key manager</a>.</p> <div class="form-row"><div class="form-group"><label for="skid">Access key</label> <input id="skid"${attr("value", spacesKeyId)}/></div> <div class="form-group"><label for="ssec">Secret key</label> <input id="ssec" type="password"${attr("value", spacesSecret)}/></div></div> <div class="form-row"><div class="form-group"><label for="sbu">Bucket name</label> <input id="sbu"${attr("value", spacesBucket)}/></div> <div class="form-group"><label for="sreg">Region</label> `);
								$$renderer.select({
									id: "sreg",
									value: spacesRegion,
									onchange: () => spacesProbed = false
								}, ($$renderer) => {
									$$renderer.push(`<!--[-->`);
									const each_array_4 = ensure_array_like(regions);
									for (let $$index_4 = 0, $$length = each_array_4.length; $$index_4 < $$length; $$index_4++) {
										let r = each_array_4[$$index_4];
										$$renderer.option({ value: r }, ($$renderer) => {
											$$renderer.push(`${escape_html(r)}`);
										});
									}
									$$renderer.push(`<!--]-->`);
								});
								$$renderer.push(`</div></div> <div class="btn-row"><button class="btn ghost"${attr("disabled", !spacesKeyId.trim(), true)}>${escape_html("Find bucket")}</button> `);
								if (spacesProbed && true) $$renderer.push(`<!--[0--><button class="btn primary"${attr("disabled", spacesCreating, true)}>${escape_html(`Create “${spacesBucket}” in ${spacesRegion}`)}</button>`);
								else $$renderer.push("<!--[-1-->");
								$$renderer.push(`<!--]--></div> `);
								$$renderer.push("<!--[-1-->");
								$$renderer.push(`<!--]-->`);
								$$renderer.push(`<!--]-->`);
							} else $$renderer.push("<!--[-1-->");
							$$renderer.push(`<!--]-->`);
						} else if (step === 5) {
							$$renderer.push(`<!--[5--><p class="hint">The on-machine coding agent. Optional — you can add a provider later in Settings.</p> <div class="option-grid"><!--[-->`);
							const each_array_6 = ensure_array_like(AGENT_PROVIDERS);
							for (let $$index_6 = 0, $$length = each_array_6.length; $$index_6 < $$length; $$index_6++) {
								let p = each_array_6[$$index_6];
								providerCard($$renderer, p, agentProvider, (id) => agentProvider = id, p.id === "openrouter" ? {
									label: "Recommended",
									kind: "rec"
								} : {
									label: "Soon",
									kind: "soon"
								});
							}
							$$renderer.push(`<!--]--></div> `);
							if (agentProvider === "openrouter") {
								$$renderer.push(`<!--[0--><div class="form-group"><label for="ork">OpenRouter API key</label> <input id="ork" type="password"${attr("value", openrouterKey)} placeholder="sk-or-… (optional)"/> <button type="button" class="hint-toggle">Where do I get this? →</button> `);
								$$renderer.push("<!--[-1-->");
								$$renderer.push(`<!--]--></div>`);
							} else $$renderer.push("<!--[-1-->");
							$$renderer.push(`<!--]-->`);
						} else if (step === 6) {
							$$renderer.push(`<!--[6--><p class="hint">Used for new machines; each profile can override.</p> <div class="form-row"><div class="form-group"><label for="reg">Region</label> `);
							$$renderer.select({
								id: "reg",
								value: region
							}, ($$renderer) => {
								$$renderer.push(`<!--[-->`);
								const each_array_7 = ensure_array_like(regions);
								for (let $$index_7 = 0, $$length = each_array_7.length; $$index_7 < $$length; $$index_7++) {
									let r = each_array_7[$$index_7];
									$$renderer.option({ value: r }, ($$renderer) => {
										$$renderer.push(`${escape_html(r)}`);
									});
								}
								$$renderer.push(`<!--]-->`);
							});
							$$renderer.push(`</div> <div class="form-group"><label for="sz">Default size</label> `);
							$$renderer.select({
								id: "sz",
								value: size
							}, ($$renderer) => {
								$$renderer.push(`<!--[-->`);
								const each_array_8 = ensure_array_like(sizes);
								for (let $$index_8 = 0, $$length = each_array_8.length; $$index_8 < $$length; $$index_8++) {
									let s = each_array_8[$$index_8];
									$$renderer.option({ value: s }, ($$renderer) => {
										$$renderer.push(`${escape_html(s)}`);
									});
								}
								$$renderer.push(`<!--]-->`);
							});
							$$renderer.push(`</div></div>`);
						} else $$renderer.push(`<!--[-1--><p class="hint">Next: create a machine profile. If you skipped a snapshot, set one on the
            profile after you snapshot a prepared builder.</p>`);
						$$renderer.push(`<!--]-->`);
					},
					$$slots: {
						heading: true,
						default: true
					}
				});
			}
			$$renderer.push(`<!----></main>`);
		}
		do {
			$$settled = true;
			$$inner_renderer = $$renderer.copy();
			$$render_inner($$inner_renderer);
		} while (!$$settled);
		$$renderer.subsume($$inner_renderer);
	});
}
//#endregion
export { _page as default };

//# sourceMappingURL=_page.svelte.js.map