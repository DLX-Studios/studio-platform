import { a as derived, f as attr, h as escape_html, i as bind_props, o as ensure_array_like, p as clsx, t as attr_class } from "../../../chunks/server2.js";
import { n as goto, r as invalidate } from "../../../chunks/navigation.js";
import { t as onDestroy } from "../../../chunks/index-server.js";
import { E as Cloud, a as Trash, l as Square, m as Play, p as Plus } from "../../../chunks/lib.js";
import { t as OptionCard } from "../../../chunks/OptionCard.js";
import { t as AppShell } from "../../../chunks/AppShell.js";
import { t as Wizard } from "../../../chunks/Wizard.js";
//#region src/lib/components/ConfirmDialog.svelte
function ConfirmDialog($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { open = false, title = "Confirm", message = "", confirmLabel = "Confirm", danger = false, requireText = "", busy = false, error = "", onconfirm } = $$props;
		let typed = "";
		const ready = derived(() => !requireText || typed.trim() === requireText);
		if (open) {
			$$renderer.push(`<!--[0--><div class="dialog-backdrop" role="presentation"><div class="card dialog" role="dialog" aria-modal="true"${attr("aria-label", title)}><div class="dialog-head"><h2>${escape_html(title)}</h2></div> `);
			if (error) $$renderer.push(`<!--[0--><div class="alert error">${escape_html(error)}</div>`);
			else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]--> `);
			if (message) $$renderer.push(`<!--[0--><p class="hint" style="margin-bottom: 1rem;">${escape_html(message)}</p>`);
			else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]--> `);
			if (requireText) $$renderer.push(`<!--[0--><div class="form-group"><label for="confirm-typed">Type <strong>${escape_html(requireText)}</strong> to confirm</label> <input id="confirm-typed"${attr("value", typed)}${attr("disabled", busy, true)}/></div>`);
			else $$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]--> <div class="wizard-nav"><button class="btn ghost"${attr("disabled", busy, true)}>Cancel</button> <button${attr_class(`btn ${danger ? "danger" : "primary"}`)}${attr("disabled", !ready() || busy, true)}>${escape_html(busy ? "Working…" : confirmLabel)}</button></div></div></div>`);
		} else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]-->`);
		bind_props($$props, { open });
	});
}
//#endregion
//#region src/routes/machines/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { data } = $$props;
		let machines = derived(() => data.machines);
		let busyId = null;
		let deleteTarget = null;
		let deleteOpen = false;
		let deleteBusy = false;
		let deleteError = "";
		let poll;
		const CSTEPS = [
			"Region",
			"Image",
			"Dedicated VM",
			"CPU",
			"Memory",
			"Storage",
			"Extra storage",
			"SSH keys",
			"Finalize"
		];
		let showCreate = false;
		let cstep = 0;
		let options = null;
		let optionsLoading = false;
		let newName = "";
		let newRegion = "";
		let newSnapshot = "";
		let newImage = "";
		let newSshIds = [];
		let newKeyName = "";
		let newKeyPub = "";
		let addingKey = false;
		let dedicated = true;
		let cpu = 0;
		let memoryGb = 0;
		let diskGb = 0;
		let volumeGb = 0;
		let newRepo = "";
		let newIdle = 0;
		let createError = "";
		let creating = false;
		const VOLUME_OPTIONS = [
			0,
			25,
			50,
			100,
			250
		];
		const VOLUME_HOURLY_PER_GB = 15e-5;
		const volumeHourly = (gb) => gb * VOLUME_HOURLY_PER_GB;
		function sizeClass(s) {
			if (s.slug.startsWith("gpu")) return "gpu";
			const d = s.description.toLowerCase();
			if (d.includes("basic") || d.includes("shared")) return "shared";
			if (d) return "dedicated";
			return /^s\d*-/.test(s.slug) ? "shared" : "dedicated";
		}
		let regionSizes = derived(() => (options?.sizes ?? []).filter((s) => s.available && s.regions.includes(newRegion) && sizeClass(s) === (dedicated ? "dedicated" : "shared")));
		let cpuOptions = derived(() => [...new Set(regionSizes().map((s) => s.vcpus))].sort((a, b) => a - b));
		let memoryOptions = derived(() => [...new Set(regionSizes().filter((s) => s.vcpus === cpu).map((s) => s.memoryGb))].sort((a, b) => a - b));
		let storageOptions = derived(() => regionSizes().filter((s) => s.vcpus === cpu && s.memoryGb === memoryGb));
		let chosenSize = derived(() => storageOptions().find((s) => s.diskGb === diskGb) ?? null);
		let distroGroups = derived(() => {
			const map = /* @__PURE__ */ new Map();
			for (const d of options?.distributions ?? []) {
				const list = map.get(d.distribution) ?? [];
				list.push(d);
				map.set(d.distribution, list);
			}
			return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
		});
		function distroVersion(d) {
			let v = d.name.replace(" x64", "").trim();
			if (v.toLowerCase().startsWith(d.distribution.toLowerCase())) v = v.slice(d.distribution.length).trim();
			return v || d.slug;
		}
		async function openCreate() {
			showCreate = true;
			cstep = 0;
			createError = "";
			newName = "";
			newRegion = "";
			newSnapshot = "";
			newImage = "";
			newKeyName = "";
			newKeyPub = "";
			dedicated = true;
			cpu = 0;
			memoryGb = 0;
			diskGb = 0;
			volumeGb = 0;
			newRepo = "";
			newIdle = 0;
			optionsLoading = true;
			try {
				const r = await fetch("/api/do/options");
				const data = (r.headers.get("content-type") ?? "").includes("application/json") ? await r.json().catch(() => null) : null;
				if (r.status === 401) {
					await goto("/login");
					return;
				}
				if (!r.ok) throw new Error(data?.error ?? `Request failed (${r.status}). Please try again.`);
				if (!data || !Array.isArray(data.regions)) throw new Error("Session expired — please log in again.");
				options = data;
				newSshIds = (data.sshKeys ?? []).map((k) => k.id);
			} catch (e) {
				createError = e.message;
			}
			optionsLoading = false;
		}
		function cNext() {
			createError = "";
			cstep = Math.min(cstep + 1, CSTEPS.length - 1);
		}
		function cCanNext() {
			switch (cstep) {
				case 0: return !!newRegion;
				case 1: return !!newSnapshot || !!newImage;
				case 3: return cpu > 0;
				case 4: return memoryGb > 0;
				case 5: return !!chosenSize();
				case 7: return newSshIds.length > 0;
				case 8: return !!newName.trim();
				default: return true;
			}
		}
		async function doDelete() {
			if (!deleteTarget) return;
			deleteBusy = true;
			deleteError = "";
			try {
				const r = await fetch(`/api/machines/${deleteTarget.id}`, {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({ action: "delete" })
				});
				if (!r.ok) {
					deleteError = (await r.json()).error ?? "Delete failed";
					return;
				}
				deleteOpen = false;
				deleteTarget = null;
				await invalidate("app:machines");
			} finally {
				deleteBusy = false;
			}
		}
		async function create() {
			creating = true;
			createError = "";
			try {
				const r = await fetch("/api/machines", {
					method: "POST",
					headers: { "Content-Type": "application/json" },
					body: JSON.stringify({
						name: newName,
						size: chosenSize()?.slug,
						region: newRegion,
						snapshotId: newSnapshot || void 0,
						imageSlug: newImage || void 0,
						repo: newRepo || void 0,
						idleTimeoutMin: newIdle,
						dedicated,
						volumeGb,
						sshKeyIds: newSshIds.join(",")
					})
				});
				const data = await r.json();
				if (!r.ok) throw new Error(data.error);
				showCreate = false;
				await invalidate("app:machines");
			} catch (e) {
				createError = e.message;
			}
			creating = false;
		}
		function dotClass(m) {
			if (!m.status.running) return "dot off";
			if (m.status.state === "starting") return "dot winding";
			if (m.status.state === "provisioning") return "dot provisioning";
			return "dot ready";
		}
		onDestroy(() => clearInterval(poll));
		let $$settled = true;
		let $$inner_renderer;
		function $$render_inner($$renderer) {
			AppShell($$renderer, {
				onnew: (kind) => kind === "machine" && openCreate(),
				children: ($$renderer) => {
					$$renderer.push(`<main class="container"><h1>Machines</h1> `);
					$$renderer.push("<!--[-1-->");
					$$renderer.push(`<!--]--> `);
					if (machines().length === 0) {
						$$renderer.push(`<!--[0--><div class="machines-grid"><div class="card machine-card empty-state fade-in">`);
						Cloud($$renderer, {
							size: 36,
							weight: "duotone"
						});
						$$renderer.push(`<!----> <p>No machines yet.</p> <button class="btn primary">`);
						Plus($$renderer, {
							size: 16,
							weight: "bold"
						});
						$$renderer.push(`<!----> Create your first machine</button></div></div>`);
					} else {
						$$renderer.push(`<!--[-1--><div class="machines-grid"><!--[-->`);
						const each_array = ensure_array_like(machines());
						for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
							let m = each_array[$$index];
							$$renderer.push(`<div${attr_class("card machine-card", void 0, { "booting": m.status.state === "starting" || m.status.state === "provisioning" })}><div class="card-head" role="button" tabindex="0"><span${attr_class(clsx(dotClass(m)))}></span> <strong>${escape_html(m.name)}</strong></div> <p class="meta">${escape_html(m.size)} · ${escape_html(m.region)} · ${escape_html(m.status.state)}</p> <div class="quick-actions">`);
							if (m.status.running) {
								$$renderer.push(`<!--[0--><button class="btn small"${attr("disabled", busyId === m.id, true)}>`);
								Square($$renderer, {
									size: 12,
									weight: "fill"
								});
								$$renderer.push(`<!----> Stop</button>`);
							} else {
								$$renderer.push(`<!--[-1--><button class="btn small primary"${attr("disabled", busyId === m.id, true)}>`);
								Play($$renderer, {
									size: 12,
									weight: "fill"
								});
								$$renderer.push(`<!----> Start</button>`);
							}
							$$renderer.push(`<!--]--> <button class="btn small danger"${attr("disabled", busyId === m.id, true)}>`);
							Trash($$renderer, { size: 12 });
							$$renderer.push(`<!----></button></div></div>`);
						}
						$$renderer.push(`<!--]--></div>`);
					}
					$$renderer.push(`<!--]--></main> `);
					if (showCreate) {
						$$renderer.push(`<!--[0--><div class="dialog-backdrop" role="presentation">`);
						{
							function heading($$renderer) {
								$$renderer.push(`<h1>${escape_html(CSTEPS[cstep])}</h1>`);
							}
							Wizard($$renderer, {
								variant: "dialog",
								title: "New machine",
								steps: CSTEPS,
								error: createError,
								nextLabel: cstep === CSTEPS.length - 1 ? "Create machine" : "Continue",
								busyLabel: "Creating…",
								busy: creating,
								canNext: cCanNext(),
								onnext: () => cstep === CSTEPS.length - 1 ? create() : cNext(),
								onclose: () => showCreate = false,
								get step() {
									return cstep;
								},
								set step($$value) {
									cstep = $$value;
									$$settled = false;
								},
								heading,
								children: ($$renderer) => {
									if (optionsLoading) $$renderer.push(`<!--[0--><div class="skeleton" style="height: 8rem;"></div>`);
									else {
										$$renderer.push("<!--[-1-->");
										if (cstep === 0) {
											$$renderer.push(`<!--[0--><p class="hint">Where should the builder run? Availability and pricing vary by region.</p> <div class="option-grid"><!--[-->`);
											const each_array_1 = ensure_array_like(options?.regions ?? []);
											for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
												let reg = each_array_1[$$index_1];
												OptionCard($$renderer, {
													selected: newRegion === reg.slug,
													onclick: () => {
														newRegion = reg.slug;
													},
													children: ($$renderer) => {
														$$renderer.push(`<strong>${escape_html(reg.name)}</strong> <span class="meta">${escape_html(reg.slug)}</span>`);
													},
													$$slots: { default: true }
												});
											}
											$$renderer.push(`<!--]--></div>`);
										} else if (cstep === 1) {
											$$renderer.push(`<!--[1--><p class="hint">Boot from a plain OS and build your own golden snapshot later, or pick an
              existing snapshot if you already have one.</p> <p class="field-label">Distributions</p> <!--[-->`);
											const each_array_2 = ensure_array_like(distroGroups());
											for (let $$index_3 = 0, $$length = each_array_2.length; $$index_3 < $$length; $$index_3++) {
												let [family, versions] = each_array_2[$$index_3];
												$$renderer.push(`<div class="distro-group"><p class="meta distro-family">${escape_html(family)}</p> <div class="version-row"><!--[-->`);
												const each_array_3 = ensure_array_like(versions);
												for (let $$index_2 = 0, $$length = each_array_3.length; $$index_2 < $$length; $$index_2++) {
													let dist = each_array_3[$$index_2];
													OptionCard($$renderer, {
														compact: true,
														selected: newImage === dist.slug,
														onclick: () => {
															newImage = dist.slug;
															newSnapshot = "";
														},
														children: ($$renderer) => {
															$$renderer.push(`<strong>${escape_html(distroVersion(dist))}</strong>`);
														},
														$$slots: { default: true }
													});
												}
												$$renderer.push(`<!--]--></div></div>`);
											}
											$$renderer.push(`<!--]--> <p class="field-label">Golden snapshots</p> `);
											if ((options?.snapshots ?? []).length === 0) $$renderer.push(`<!--[0--><p class="hint">None on this account yet. Pick a distribution above, then take a snapshot of the running machine — it will show up here next time.</p>`);
											else {
												$$renderer.push(`<!--[-1--><div class="option-grid"><!--[-->`);
												const each_array_4 = ensure_array_like(options?.snapshots ?? []);
												for (let $$index_4 = 0, $$length = each_array_4.length; $$index_4 < $$length; $$index_4++) {
													let snap = each_array_4[$$index_4];
													OptionCard($$renderer, {
														selected: newSnapshot === snap.id,
														onclick: () => {
															newSnapshot = snap.id;
															newImage = "";
														},
														children: ($$renderer) => {
															$$renderer.push(`<strong>${escape_html(snap.name)}</strong> <span class="meta">snapshot ${escape_html(snap.id)}</span>`);
														},
														$$slots: { default: true }
													});
												}
												$$renderer.push(`<!--]--></div>`);
											}
											$$renderer.push(`<!--]-->`);
										} else if (cstep === 2) {
											$$renderer.push(`<!--[2--><p class="hint">Dedicated VMs give you vCPUs that no other customer shares.</p> <div class="option-grid two">`);
											OptionCard($$renderer, {
												selected: !dedicated,
												onclick: () => {
													dedicated = false;
												},
												children: ($$renderer) => {
													$$renderer.push(`<strong>Shared CPU</strong> <span class="meta">Basic droplets — oversubscribed vCPUs, cheapest</span>`);
												},
												$$slots: { default: true }
											});
											$$renderer.push(`<!----> `);
											OptionCard($$renderer, {
												selected: dedicated,
												onclick: () => {
													dedicated = true;
												},
												children: ($$renderer) => {
													$$renderer.push(`<strong>Dedicated CPU</strong> <span class="meta">CPU-optimized — reserved vCPUs, best for builds</span>`);
												},
												$$slots: { default: true }
											});
											$$renderer.push(`<!----></div>`);
										} else if (cstep === 3) {
											$$renderer.push(`<!--[3--><p class="hint">vCPUs available in ${escape_html(newRegion)} for ${escape_html(dedicated ? "dedicated" : "shared")} machines.</p> `);
											if (cpuOptions().length === 0) $$renderer.push(`<!--[0--><div class="empty-state" style="padding: 1.5rem;"><p>No ${escape_html(dedicated ? "dedicated" : "shared")} plans in ${escape_html(newRegion)}.</p> <p class="meta">Go back to pick another region, or switch the Dedicated VM choice.</p></div>`);
											else {
												$$renderer.push(`<!--[-1--><div class="option-grid"><!--[-->`);
												const each_array_5 = ensure_array_like(cpuOptions());
												for (let $$index_5 = 0, $$length = each_array_5.length; $$index_5 < $$length; $$index_5++) {
													let c = each_array_5[$$index_5];
													OptionCard($$renderer, {
														selected: cpu === c,
														onclick: () => {
															cpu = c;
															memoryGb = 0;
															diskGb = 0;
														},
														children: ($$renderer) => {
															$$renderer.push(`<strong>${escape_html(c)} vCPU</strong>`);
														},
														$$slots: { default: true }
													});
												}
												$$renderer.push(`<!--]--></div>`);
											}
											$$renderer.push(`<!--]-->`);
										} else if (cstep === 4) {
											$$renderer.push(`<!--[4--><p class="hint">Memory options for ${escape_html(cpu)} vCPU in ${escape_html(newRegion)}.</p> <div class="option-grid"><!--[-->`);
											const each_array_6 = ensure_array_like(memoryOptions());
											for (let $$index_6 = 0, $$length = each_array_6.length; $$index_6 < $$length; $$index_6++) {
												let mem = each_array_6[$$index_6];
												OptionCard($$renderer, {
													selected: memoryGb === mem,
													onclick: () => {
														memoryGb = mem;
														diskGb = 0;
													},
													children: ($$renderer) => {
														$$renderer.push(`<strong>${escape_html(mem)} GB</strong>`);
													},
													$$slots: { default: true }
												});
											}
											$$renderer.push(`<!--]--></div>`);
										} else if (cstep === 5) {
											$$renderer.push(`<!--[5--><p class="hint">Storage is bundled with the plan — pick the disk size that fits.</p> <div class="option-grid"><!--[-->`);
											const each_array_7 = ensure_array_like(storageOptions());
											for (let $$index_7 = 0, $$length = each_array_7.length; $$index_7 < $$length; $$index_7++) {
												let s = each_array_7[$$index_7];
												OptionCard($$renderer, {
													selected: diskGb === s.diskGb,
													onclick: () => diskGb = s.diskGb,
													children: ($$renderer) => {
														$$renderer.push(`<strong>${escape_html(s.diskGb)} GB SSD</strong> <span class="meta">$${escape_html(s.hourly.toFixed(3))}/hr · $${escape_html(s.monthly)}/mo</span>`);
													},
													$$slots: { default: true }
												});
											}
											$$renderer.push(`<!--]--></div> `);
											if (chosenSize()) $$renderer.push(`<!--[0--><p class="meta">Plan: <code>${escape_html(chosenSize().slug)}</code></p>`);
											else $$renderer.push("<!--[-1-->");
											$$renderer.push(`<!--]-->`);
										} else if (cstep === 6) {
											$$renderer.push(`<!--[6--><p class="hint">Additional block storage is a persistent volume — it survives Stop/Start
              cycles and costs ~$0.10/GB per month while it exists.</p> <div class="option-grid"><!--[-->`);
											const each_array_8 = ensure_array_like(VOLUME_OPTIONS);
											for (let $$index_8 = 0, $$length = each_array_8.length; $$index_8 < $$length; $$index_8++) {
												let gb = each_array_8[$$index_8];
												OptionCard($$renderer, {
													selected: volumeGb === gb,
													onclick: () => volumeGb = gb,
													children: ($$renderer) => {
														$$renderer.push(`<strong>${escape_html(gb === 0 ? "None" : `${gb} GB`)}</strong> `);
														if (gb > 0) $$renderer.push(`<!--[0--><span class="meta">~$${escape_html(volumeHourly(gb).toFixed(4))}/hr · $${escape_html((gb * 720 * VOLUME_HOURLY_PER_GB).toFixed(2))}/mo</span>`);
														else $$renderer.push("<!--[-1-->");
														$$renderer.push(`<!--]-->`);
													},
													$$slots: { default: true }
												});
											}
											$$renderer.push(`<!--]--></div>`);
										} else if (cstep === 7) {
											$$renderer.push(`<!--[7--><p class="hint">Keys are injected into the droplet at boot. Select from your account, or
              paste a new public key to create one.</p> <!--[-->`);
											const each_array_9 = ensure_array_like(options?.sshKeys ?? []);
											for (let $$index_9 = 0, $$length = each_array_9.length; $$index_9 < $$length; $$index_9++) {
												let key = each_array_9[$$index_9];
												$$renderer.push(`<label class="check-row"><input type="checkbox"${attr("checked", newSshIds.includes(key.id), true)}/> ${escape_html(key.name)} <span class="meta">${escape_html(key.fingerprint)}</span></label>`);
											}
											$$renderer.push(`<!--]--> `);
											if (newSshIds.length === 0) $$renderer.push(`<!--[0--><p class="hint">Select at least one key — otherwise you can't SSH in.</p>`);
											else $$renderer.push("<!--[-1-->");
											$$renderer.push(`<!--]--> <div class="form-group" style="margin-top: 1rem;"><label for="nkey">Create a key</label> <p class="hint" style="margin-bottom: 0.5rem;"><strong>Generate</strong> creates an ed25519 pair on the dashboard server
                (private half stays at <code>data/ssh-keys/</code>), imports the public half
                to DigitalOcean, and selects it. <strong>Paste</strong> imports your own public key.</p> <input id="nkey"${attr("value", newKeyPub)} placeholder="paste a public key: ssh-ed25519 AAAA… user@host"/> <div class="btn-row" style="margin-top: 0.5rem;"><input style="max-width: 12rem;"${attr("value", newKeyName)} placeholder="name (optional — auto-generated)"${attr("disabled", addingKey, true)}/> <button class="btn small" type="button"${attr("disabled", addingKey, true)}>${escape_html("Generate on server")}</button> <button class="btn small" type="button"${attr("disabled", !newKeyPub.trim(), true)}>${escape_html("Import pasted key")}</button></div> `);
											$$renderer.push("<!--[-1-->");
											$$renderer.push(`<!--]--></div>`);
										} else if (cstep === 8) {
											$$renderer.push(`<!--[8--><div class="form-group"><label for="mname">Name</label> <input id="mname"${attr("value", newName)} placeholder="big-build"/></div> <div class="form-group"><label for="midle">Idle auto-destroy (min, 0 = off)</label> <input id="midle" type="number" min="0"${attr("value", newIdle)}/></div> <div class="form-group"><label for="mrepo">Repository URL (optional)</label> <input id="mrepo"${attr("value", newRepo)} placeholder="https://github.com/owner/repo"/></div> <div class="review"><p><span>Region</span> <strong>${escape_html(newRegion)}</strong></p> <p><span>Image</span> <strong>${escape_html(newSnapshot ? `snapshot ${newSnapshot}` : newImage || "none yet")}</strong></p> <p><span>Plan</span> <strong>${escape_html(chosenSize() ? `${chosenSize().slug} — ${chosenSize().vcpus} vCPU · ${chosenSize().memoryGb} GB RAM · ${chosenSize().diskGb} GB SSD` : "—")}</strong></p> <p><span>Extra storage</span> <strong>${escape_html(volumeGb > 0 ? `${volumeGb} GB volume` : "none")}</strong></p> <p><span>SSH keys</span> <strong>${escape_html(newSshIds.length)} selected</strong></p> <p><span>Options</span> <strong>No backups · IPv4 only · No monitoring${escape_html(dedicated ? " · Dedicated VM" : "")}</strong></p> `);
											if (chosenSize()) $$renderer.push(`<!--[0--><p><span>Cost</span> <strong>$${escape_html((chosenSize().hourly + volumeHourly(volumeGb)).toFixed(4))}/hr</strong></p> <p class="meta" style="justify-content: flex-end;">plan $${escape_html(chosenSize().hourly.toFixed(4))}/hr${escape_html(volumeGb > 0 ? ` + volume $${volumeHourly(volumeGb).toFixed(4)}/hr` : "")}</p>`);
											else $$renderer.push("<!--[-1-->");
											$$renderer.push(`<!--]--></div>`);
										} else $$renderer.push("<!--[-1-->");
										$$renderer.push(`<!--]-->`);
									}
									$$renderer.push(`<!--]-->`);
								},
								$$slots: {
									heading: true,
									default: true
								}
							});
						}
						$$renderer.push(`<!----></div>`);
					} else $$renderer.push("<!--[-1-->");
					$$renderer.push(`<!--]--> `);
					ConfirmDialog($$renderer, {
						title: "Delete machine",
						message: deleteTarget ? `This destroys the droplet and volume on DigitalOcean and removes the profile “${deleteTarget.name}” (id ${deleteTarget.id}). This cannot be undone.` : "",
						confirmLabel: "Delete machine",
						danger: true,
						requireText: deleteTarget?.name ?? "",
						busy: deleteBusy,
						error: deleteError,
						onconfirm: doDelete,
						get open() {
							return deleteOpen;
						},
						set open($$value) {
							deleteOpen = $$value;
							$$settled = false;
						}
					});
					$$renderer.push(`<!---->`);
				},
				$$slots: { default: true }
			});
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