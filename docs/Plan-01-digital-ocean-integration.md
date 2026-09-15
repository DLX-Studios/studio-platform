# Plan 01 — DigitalOcean Integration

Integrate DigitalOcean end-to-end: credentials, SSH keys, machine building
(golden snapshots), Spaces, networking, and billing — so the dashboard can
manage a builder's full lifecycle without ever opening the DO console.

**Status legend:** ✅ done · 🔨 next · 🕓 later · ❓ open question

---

## 0. Current state (baseline)

| Area | State | Where |
|---|---|---|
| SDK access | ✅ `@digitalocean/dots` via dynamic loader (`sdkLoader.ts`), token from SQLite | `dashboard/src/lib/doClient.ts` |
| Droplet CRUD | ✅ list (all/by-tag), create (user-data, tags, ssh keys), delete | `doClient.ts`, `api/machines/[id]` |
| Token validation | ✅ live probe with reject/network/rate classification | `api/setup/probe` |
| SSH keys | ⚠️ list + pick only; keys must be added in DO console by hand | `api/setup/probe` |
| Snapshots | ⚠️ list + pick only; golden image creation is manual | `scripts/setup-builder.sh` |
| Spaces | ⚠️ bucket + keys entered manually; used only by sccache over S3 | wizard step 4, cloud-init |
| Regions / sizes | ⚠️ hardcoded arrays in wizard & machine dialog | `setup/+page.svelte` |
| Pricing | ⚠️ single hardcoded `cost_per_hour` setting | settings |
| Firewalls / VPC | ❌ nothing — builders are wide open | — |
| Billing | ❌ estimates only | — |

---

## 1. Account & SDK foundations

Small gaps that everything else builds on.

- 🔨 **Account panel data**: surface `v2.account.get()` (email, droplet limit,
  email_verified) in Settings; block machine start when
  `droplets >= droplet_limit` with a clear message.
- 🔨 **Action polling helper**: every DO mutation returns an *action*;
  add `waitForAction(dropletId, actionId)` that polls
  `v2.droplets.byDroplet_id(id).actions.byAction_id(id).get()` until
  `completed`/`errored`. Reuse everywhere (power off, snapshot, resize…).
- 🔨 **Rate-limit awareness**: read `ratelimit-remaining` handling off the
  SDK errors (`reason: 'rate'` already exists) and back off the dashboard's
  5s status poller instead of hammering.
- 🕓 **Region/size catalogs**: replace hardcoded lists with
  `v2.regions.get()` (filter `available: true`) and `v2.sizes.get()` —
  this also brings **real `priceHourly`** per size (kills `COST_PER_HOUR`).

**Acceptance:** wizard dropdowns are API-fed; starting a machine fails
gracefully at the account limit.

---

## 2. SSH keys — the dashboard owns a keypair

Today the dashboard borrows the user's key IDs. That couples builders to the
user's laptop and blocks the daemon tunnel design (the *dashboard server*
needs root SSH to builders, not the user's machine).

- 🔨 **Dedicated builder keypair**: on first run (or Settings), generate an
  ed25519 keypair locally, import the public half via
  `v2.account.keys.post({ name: 'studio-builder-<hostname>', publicKey })`,
  store the private half in SQLite (file perms `0600` on the DB dir).
  All machines get this key automatically; the user's own keys stay optional
  convenience entries.
- 🔨 **Import flow in wizard**: "paste a public key" → `keys.post` — no more
  console trips for users who want their own key too.
- 🔨 **Key management in Settings**: list (`v2.account.keys.get()`), add,
  delete (`v2.account.keys.bySsh_key_id(id).delete()`), with a guard that
  refuses to delete the key a running machine depends on.
- ❓ Per-machine extra keys (e.g. a second laptop)? Default: no — one global
  set, selectable per machine profile later if needed.

**Acceptance:** fresh wizard on a fresh DO account → dashboard generates,
imports, and uses its own key; user never touches the SSH-keys console.

---

## 3. Machine building — automate the golden snapshot

Replace the manual *run script → poweroff → console snapshot* dance with a
"**Forge snapshot**" flow driven entirely by the dashboard.

- 🔨 **Forge pipeline** (Settings → Snapshots → Forge):
  1. Create temp droplet `snapshot-forge-<ts>` (smallest size, c-2), user-data
     = `scripts/setup-builder.sh` (moved into the dashboard so it's versioned
     with the app), tagged `sb-forge`.
  2. Wait for `active`, then wait for cloud-init completion — marker file
     `cloud-init status --wait` polled over SSH (the dashboard's keypair from
     §2 makes this possible) or a retry-until-success probe.
  3. Power off via action (`type: 'power_off'`), wait via `waitForAction`.
  4. Snapshot via `v2.droplets.byDroplet_id(id).actions.post({ type: 'snapshot', name: 'studio-golden-<version>' })`.
  5. Delete the temp droplet (money stops here).
  6. Optionally promote the new snapshot to the default in settings.
- 🔨 **Snapshot management page**: list (`v2.snapshots.get`) with size, created
  date, regions; delete (`v2.snapshots.bySnapshot_id(id).delete()`); show
  `min_disk_size` warning when a snapshot can't back a chosen size.
- 🔨 **Retention**: keep last N golden snapshots (default 3), prune on forge.
- 🕓 **Per-machine variants**: snapshot baked with extra targets
  (wasm32, Android NDK) — profiles pick a snapshot; forge accepts extra
  `rustup target` list as a build parameter.

**Acceptance:** from a clean DO account, "Forge" produces a working golden
snapshot with zero console interaction; machine start uses it; money for the
forge droplet stops the moment the snapshot exists.

---

## 4. Spaces — S3 integration for cache + artifacts

The dots SDK does **not** cover Spaces (it's S3-compatible, separate API).
Use a lightweight S3 client (`aws4fetch` — no deps, Bun-compatible) against
`<region>.digitaloceanspaces.com`.

- 🔨 **S3 wrapper**: `spaces.ts` with signed request helpers (PUT/GET/LIST/
  DELETE) using the stored Spaces keys — no AWS SDK weight.
- 🔨 **Bucket bootstrap**: on wizard finish, verify bucket exists
  (HEAD bucket); offer to create it if missing (`PUT /` on the bucket host).
  Kills the "create it in the console first" hint.
- 🔨 **Cache analytics**: LIST objects under the sccache prefix, aggregate
  count + size, show on Analytics (hit rate comes from the daemon later).
- 🕓 **Per-machine prefixes**: `sccache/<machine-id>/` so profiles don't
  thrash one another's cache; setting to share a prefix for related profiles.
- 🕓 **Lifecycle**: expire objects older than N days (S3 lifecycle config)
  to keep storage costs flat.
- 🕓 **Build artifacts**: push `target/` release artifacts or test outputs to
  Spaces from the daemon for download from the dashboard.

**Acceptance:** wizard creates/validates the bucket; Analytics shows real
cache size; destroying machines never loses cache.

---

## 5. Networking & security

Ephemeral public boxes running build scripts deserve real guardrails.

- 🔨 **Cloud firewall per builder** (`v2.firewalls`): inbound deny-all except
  22/tcp (optionally restricted to the dashboard's current public IP);
  outbound allow 80/443 (git, crates.io) + DNS. Attach at droplet creation
  (`firewalls` in the create body or droplet-tag-based firewall).
- 🔨 **Forge droplets get the same firewall** (they run third-party install
  scripts).
- 🕓 **Reserved IP per machine profile** (`v2.reserved_ips`): stable IP across
  destroy/create cycles → stable `known_hosts`, stable firewall allow-list.
  Costs ~$4/mo while reserved — per-profile toggle, off by default.
- 🕓 **VPC** (`v2.vpcs`): private networking between dashboard-host and
  builders if the dashboard is ever deployed on DO itself (see §8).
- ❓ Restrict outbound to an allow-list (crates.io, github/gitlab, DO
  endpoints)? Breaks ad-hoc installs — default no.

**Acceptance:** a fresh builder exposes only SSH; port scan shows nothing
else; forge and builder droplets are firewall-tagged.

---

## 6. Lifecycle, budgets & real billing

- 🔨 **Real pricing** (from §1 sizes) flows into session ledger + analytics —
  no more `cost_per_hour` setting.
- 🔨 **Balance check**: `v2.customers.my.balance.get()` on Overview — show
  account balance, warn/stop starts under a configurable floor.
- 🔨 **Session budget guard**: per-machine monthly cap (sum of session costs);
  refusal to start past cap with an override checkbox.
- 🔨 **Snapshot storage cost**: snapshots bill ~$0.05/GB-mo — include in
  analytics totals (size known from `v2.snapshots`).
- 🕓 **Auto-destroy enforcement** (daemon, Phase 2 of app roadmap): idle
  detection feeds `end_reason='idle'`; this plan provides the DO-side
  destroy call + ledger plumbing (already exists).
- 🕓 **Graceful shutdown**: optional `shutdown` action + wait + destroy for
  machines where the daemon reports in-flight builds.

**Acceptance:** Analytics matches the DO invoice within rounding; a machine
refuses to start when balance/budget says no.

---

## 7. SDK / code notes (for whoever implements)

- Dynamic loader stays (`sdkLoader.ts`) — never import the bare specifier.
- The SDK has **no Spaces support** — §4 is deliberately `aws4fetch`.
- All list endpoints accept `perPage`; DO pages at 200 max — use the SDK's
  paginator or a `links.next` loop where >200 is plausible (snapshots, keys).
- Firewalls/reserved-ips/balance request builders follow the same
  `v2.<resource>` pattern; extend the *typed wrapper functions* in
  `doClient.ts`, keep routes consuming plain shapes.
- Cloud-init templates stay in `src/lib/*.yml?raw` — the forge pipeline
  imports `setup-builder.sh` the same way (`?raw` into user-data).
- Secrets (SSH private key, Spaces secret, PATs) live only in SQLite;
  never in `user_data` unless the builder itself needs them (Spaces keys,
  OpenRouter key — already the case).

---

## 8. Later / open questions

- 🕓 Deploy the dashboard itself to a small DO droplet (phone access from
  anywhere) → HTTPS via Caddy, PIN auth already in place, daemon dials home
  (reverse tunnel) instead of dashboard-dials-builder.
- 🕓 Object-storage-backed `target/` sharing between profiles (huge cache
  wins for branch-hopping).
- 🕓 Volumes (`v2.volumes`) for a persistent scratch disk per machine profile
  (survives destroy, attaches on create) — alternative/complement to Spaces.
- ❓ Do we manage DO *teams* (multi-account) or hard-assume one account?
  Default: one account, one token.
- ❓ Container registry (`v2.registry`) for shipping the daemon binary as an
  image instead of baking into snapshots?

---

## 9. Suggested order of execution

| # | Item | Depends on | Effort |
|---|---|---|---|
| 1 | §1 regions/sizes/pricing from API | — | S |
| 2 | §2 dashboard keypair + key import | — | M |
| 3 | §3 forge pipeline | #1, #2 | L |
| 4 | §4 S3 wrapper + bucket bootstrap + cache analytics | — | M |
| 5 | §5 cloud firewalls | #2 | M |
| 6 | §6 balance + budgets + real costs | #1 | M |
| 7 | §5 reserved IPs / VPC | #5 | S |
| 8 | §4/§6 later items | as needed | — |
