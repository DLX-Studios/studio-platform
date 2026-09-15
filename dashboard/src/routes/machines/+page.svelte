<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto, invalidate } from '$app/navigation';
  import { Plus, Play, Square, Trash, Cloud } from 'phosphor-svelte';
  import AppShell from '#lib/components/AppShell.svelte';
  import Wizard from '#lib/components/Wizard.svelte';
  import OptionCard from '#lib/components/OptionCard.svelte';
  import ConfirmDialog from '#lib/components/ConfirmDialog.svelte';

  let { data } = $props();

  interface MachineRow {
    id: number;
    name: string;
    size: string;
    region: string;
    status: { running: boolean; state: string; ip: string | null };
  }

  // Single source of truth: the server load. The 5s poll just invalidates it,
  // so SSR and client updates can never diverge.
  let machines = $derived(data.machines);
  let busyId = $state<number | null>(null);
  let actionError = $state('');
  let deleteTarget = $state<{ id: number; name: string } | null>(null);
  let deleteOpen = $state(false);
  let deleteBusy = $state(false);
  let deleteError = $state('');
  let poll: ReturnType<typeof setInterval>;

  // create dialog state — DO-style step flow
  interface DoRegion {
    slug: string;
    name: string;
  }
  interface DoSize {
    slug: string;
    vcpus: number;
    memoryGb: number;
    diskGb: number;
    hourly: number;
    monthly: number;
    regions: string[];
    available: boolean;
    description: string;
  }

  const CSTEPS = ['Region', 'Image', 'Dedicated VM', 'CPU', 'Memory', 'Storage', 'Extra storage', 'SSH keys', 'Finalize'];

  let showCreate = $state(false);
  let cstep = $state(0);
  let options = $state<{ regions: DoRegion[]; sizes: DoSize[]; snapshots: { id: string; name: string }[]; sshKeys: { id: number; name: string; fingerprint?: string }[]; distributions: { id: string; slug: string; name: string; distribution: string }[] } | null>(null);
  let optionsLoading = $state(false);

  let newName = $state('');
  let newRegion = $state('');
  let newSnapshot = $state('');
  let newImage = $state('');
  let newSshIds = $state<number[]>([]);
  let newKeyName = $state('');
  let newKeyPub = $state('');
  let addingKey = $state(false);
  let keyStatus = $state('');
  let dedicated = $state(true);
  let cpu = $state(0);
  let memoryGb = $state(0);
  let diskGb = $state(0);
  let volumeGb = $state(0);
  let newRepo = $state('');
  let newIdle = $state(0);
  let createError = $state('');
  let creating = $state(false);

  const VOLUME_OPTIONS = [0, 25, 50, 100, 250];
  const VOLUME_HOURLY_PER_GB = 0.00015;
  const volumeHourly = (gb: number) => gb * VOLUME_HOURLY_PER_GB;

  function sizeClass(s: DoSize): 'shared' | 'dedicated' | 'gpu' {
    if (s.slug.startsWith('gpu')) return 'gpu';
    const d = s.description.toLowerCase();
    if (d.includes('basic') || d.includes('shared')) return 'shared';
    if (d) return 'dedicated';
    return /^s\d*-/.test(s.slug) ? 'shared' : 'dedicated';
  }

  let regionSizes = $derived(
    (options?.sizes ?? []).filter(
      (s) => s.available && s.regions.includes(newRegion) && sizeClass(s) === (dedicated ? 'dedicated' : 'shared'),
    ),
  );
  let cpuOptions = $derived([...new Set(regionSizes.map((s) => s.vcpus))].sort((a, b) => a - b));
  let memoryOptions = $derived(
    [...new Set(regionSizes.filter((s) => s.vcpus === cpu).map((s) => s.memoryGb))].sort((a, b) => a - b),
  );
  let storageOptions = $derived(regionSizes.filter((s) => s.vcpus === cpu && s.memoryGb === memoryGb));
  let chosenSize = $derived(storageOptions.find((s) => s.diskGb === diskGb) ?? null);

  let distroGroups = $derived.by(() => {
    const map = new Map<string, { id: string; slug: string; name: string; distribution: string }[]>();
    for (const d of options?.distributions ?? []) {
      const list = map.get(d.distribution) ?? [];
      list.push(d);
      map.set(d.distribution, list);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  });

  function distroVersion(d: { name: string; distribution: string; slug: string }): string {
    let v = d.name.replace(' x64', '').trim();
    if (v.toLowerCase().startsWith(d.distribution.toLowerCase())) v = v.slice(d.distribution.length).trim();
    return v || d.slug;
  }

  async function openCreate() {
    showCreate = true;
    cstep = 0;
    createError = '';
    newName = '';
    newRegion = '';
    newSnapshot = '';
    newImage = '';
    newKeyName = '';
    newKeyPub = '';
    dedicated = true;
    cpu = 0;
    memoryGb = 0;
    diskGb = 0;
    volumeGb = 0;
    newRepo = '';
    newIdle = 0;
    optionsLoading = true;
    try {
      const r = await fetch('/api/do/options');
      // The session cookie can expire while the page sits open. The server
      // returns JSON 401 for /api/* now, but guard against any non-JSON
      // (e.g. a proxied login page) so we never show "Unexpected token '<'".
      const ct = r.headers.get('content-type') ?? '';
      const data = ct.includes('application/json')
        ? await r.json().catch(() => null)
        : null;
      if (r.status === 401) {
        await goto('/login');
        return;
      }
      if (!r.ok) throw new Error(data?.error ?? `Request failed (${r.status}). Please try again.`);
      if (!data || !Array.isArray(data.regions)) {
        throw new Error('Session expired — please log in again.');
      }
      options = data;
      newSshIds = (data.sshKeys ?? []).map((k: { id: number }) => k.id);
    } catch (e) {
      createError = (e as Error).message;
    }
    optionsLoading = false;
  }

  function cBack() {
    createError = '';
    cstep = Math.max(0, cstep - 1);
  }

  async function addSshKey(mode: 'generate' | 'paste') {
    addingKey = true;
    createError = '';
    keyStatus = '';
    // Names are optional for both modes — mint one so the buttons always work.
    let keyName = newKeyName.trim();
    if (!keyName) keyName = `key-${Math.random().toString(36).slice(2, 7)}`;
    try {
      const url = mode === 'generate' ? '/api/do/sshkeys/generate' : '/api/do/sshkeys';
      const body =
        mode === 'generate'
          ? { name: keyName }
          : { name: keyName, publicKey: newKeyPub };
      const r = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = await r.json();
      if (!r.ok) throw new Error(data.error);
      if (options) {
        options = { ...options, sshKeys: [...options.sshKeys, data.key] };
      }
      newSshIds = [...newSshIds, data.key.id];
      keyStatus = `Added “${data.key.name}” — selected for this machine.`;
      if (mode === 'paste') newKeyPub = '';
      newKeyName = '';
    } catch (e) {
      keyStatus = (e as Error).message;
    }
    addingKey = false;
  }

  function cNext() {
    createError = '';
    cstep = Math.min(cstep + 1, CSTEPS.length - 1);
  }

  function cCanNext(): boolean {
    switch (cstep) {
      case 0:
        return !!newRegion;
      case 1:
        return !!newSnapshot || !!newImage;
      case 3:
        return cpu > 0;
      case 4:
        return memoryGb > 0;
      case 5:
        return !!chosenSize;
      case 7:
        return newSshIds.length > 0;
      case 8:
        return !!newName.trim();
      default:
        return true;
    }
  }

  async function act(id: number, action: 'start' | 'stop' | 'delete') {
    busyId = id;
    actionError = '';
    try {
      const r = await fetch(`/api/machines/${id}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      if (!r.ok) {
        const d = await r.json();
        actionError = d.error ?? 'Action failed';
      }
      await invalidate('app:machines');
    } finally {
      busyId = null;
    }
  }

  function requestDelete(m: MachineRow) {
    deleteTarget = { id: m.id, name: m.name };
    deleteError = '';
    deleteOpen = true;
  }

  async function doDelete() {
    if (!deleteTarget) return;
    deleteBusy = true;
    deleteError = '';
    try {
      const r = await fetch(`/api/machines/${deleteTarget.id}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'delete' }),
      });
      if (!r.ok) {
        const d = await r.json();
        deleteError = d.error ?? 'Delete failed';
        return;
      }
      deleteOpen = false;
      deleteTarget = null;
      await invalidate('app:machines');
    } finally {
      deleteBusy = false;
    }
  }

  async function create() {
    creating = true;
    createError = '';
    try {
      const r = await fetch('/api/machines', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: newName,
          size: chosenSize?.slug,
          region: newRegion,
          snapshotId: newSnapshot || undefined,
          imageSlug: newImage || undefined,
          repo: newRepo || undefined,
          idleTimeoutMin: newIdle,
          dedicated,
          volumeGb,
          sshKeyIds: newSshIds.join(','),
        }),
      });
      const data = await r.json();
      if (!r.ok) throw new Error(data.error);
      showCreate = false;
      await invalidate('app:machines');
    } catch (e) {
      createError = (e as Error).message;
    }
    creating = false;
  }

  function dotClass(m: MachineRow): string {
    if (!m.status.running) return 'dot off';
    if (m.status.state === 'starting') return 'dot winding';
    if (m.status.state === 'provisioning') return 'dot provisioning';
    return 'dot ready';
  }

  onMount(() => {
    poll = setInterval(() => invalidate('app:machines'), 5000);
  });
  onDestroy(() => clearInterval(poll));
</script>

<AppShell onnew={(kind) => kind === 'machine' && openCreate()}>
  <main class="container">
    <h1>Machines</h1>

    {#if actionError}<div class="alert error">{actionError}</div>{/if}

  {#if machines.length === 0}
    <div class="machines-grid">
      <div class="card machine-card empty-state fade-in">
        <Cloud size={36} weight="duotone" />
        <p>No machines yet.</p>
        <button class="btn primary" onclick={openCreate}>
          <Plus size={16} weight="bold" /> Create your first machine
        </button>
      </div>
    </div>
  {:else}
    <div class="machines-grid">
      {#each machines as m (m.id)}
        <div class="card machine-card" class:booting={m.status.state === 'starting' || m.status.state === 'provisioning'}>
          <div class="card-head" onclick={() => goto(`/m/${m.id}`)} role="button" tabindex="0"
               onkeydown={(e) => e.key === 'Enter' && goto(`/m/${m.id}`)}>
            <span class={dotClass(m)}></span>
            <strong>{m.name}</strong>
          </div>
          <p class="meta">{m.size} · {m.region} · {m.status.state}</p>
          <div class="quick-actions">
            {#if m.status.running}
              <button class="btn small" onclick={() => act(m.id, 'stop')} disabled={busyId === m.id}>
                <Square size={12} weight="fill" /> Stop
              </button>
            {:else}
              <button class="btn small primary" onclick={() => act(m.id, 'start')} disabled={busyId === m.id}>
                <Play size={12} weight="fill" /> Start
              </button>
            {/if}
            <button class="btn small danger" onclick={() => requestDelete(m)} disabled={busyId === m.id}>
              <Trash size={12} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
  </main>

{#if showCreate}
  <div class="dialog-backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && (showCreate = false)}>
    <Wizard
      variant="dialog"
      title="New machine"
      steps={CSTEPS}
      bind:step={cstep}
      error={createError}
      nextLabel={cstep === CSTEPS.length - 1 ? 'Create machine' : 'Continue'}
      busyLabel="Creating…"
      busy={creating}
      canNext={cCanNext()}
      onnext={() => (cstep === CSTEPS.length - 1 ? create() : cNext())}
      onclose={() => (showCreate = false)}
    >
      {#snippet heading()}
        <h1>{CSTEPS[cstep]}</h1>
      {/snippet}

      {#if optionsLoading}
        <div class="skeleton" style="height: 8rem;"></div>
      {:else}
          {#if cstep === 0}
            <p class="hint">Where should the builder run? Availability and pricing vary by region.</p>
            <div class="option-grid">
              {#each options?.regions ?? [] as reg (reg.slug)}
                <OptionCard selected={newRegion === reg.slug} onclick={() => { newRegion = reg.slug; }}>
                  <strong>{reg.name}</strong>
                  <span class="meta">{reg.slug}</span>
                </OptionCard>
              {/each}
            </div>

          {:else if cstep === 1}
            <p class="hint">
              Boot from a plain OS and build your own golden snapshot later, or pick an
              existing snapshot if you already have one.
            </p>
            <p class="field-label">Distributions</p>
            {#each distroGroups as [family, versions] (family)}
              <div class="distro-group">
                <p class="meta distro-family">{family}</p>
                <div class="version-row">
                  {#each versions as dist (dist.id)}
                    <OptionCard
                      compact
                      selected={newImage === dist.slug}
                      onclick={() => { newImage = dist.slug; newSnapshot = ''; }}
                    >
                      <strong>{distroVersion(dist)}</strong>
                    </OptionCard>
                  {/each}
                </div>
              </div>
            {/each}

            <p class="field-label">Golden snapshots</p>
            {#if (options?.snapshots ?? []).length === 0}
              <p class="hint">None on this account yet. Pick a distribution above, then take a snapshot of the running machine — it will show up here next time.</p>
            {:else}
              <div class="option-grid">
                {#each options?.snapshots ?? [] as snap (snap.id)}
                  <OptionCard
                    selected={newSnapshot === snap.id}
                    onclick={() => { newSnapshot = snap.id; newImage = ''; }}
                  >
                    <strong>{snap.name}</strong>
                    <span class="meta">snapshot {snap.id}</span>
                  </OptionCard>
                {/each}
              </div>
            {/if}

          {:else if cstep === 2}
            <p class="hint">Dedicated VMs give you vCPUs that no other customer shares.</p>
            <div class="option-grid two">
              <OptionCard selected={!dedicated} onclick={() => { dedicated = false; }}>
                <strong>Shared CPU</strong>
                <span class="meta">Basic droplets — oversubscribed vCPUs, cheapest</span>
              </OptionCard>
              <OptionCard selected={dedicated} onclick={() => { dedicated = true; }}>
                <strong>Dedicated CPU</strong>
                <span class="meta">CPU-optimized — reserved vCPUs, best for builds</span>
              </OptionCard>
            </div>

          {:else if cstep === 3}
            <p class="hint">vCPUs available in {newRegion} for {dedicated ? 'dedicated' : 'shared'} machines.</p>
            {#if cpuOptions.length === 0}
              <div class="empty-state" style="padding: 1.5rem;">
                <p>No {dedicated ? 'dedicated' : 'shared'} plans in {newRegion}.</p>
                <p class="meta">Go back to pick another region, or switch the Dedicated VM choice.</p>
              </div>
            {:else}
              <div class="option-grid">
                {#each cpuOptions as c (c)}
                  <OptionCard selected={cpu === c} onclick={() => { cpu = c; memoryGb = 0; diskGb = 0; }}>
                    <strong>{c} vCPU</strong>
                  </OptionCard>
                {/each}
              </div>
            {/if}

          {:else if cstep === 4}
            <p class="hint">Memory options for {cpu} vCPU in {newRegion}.</p>
            <div class="option-grid">
              {#each memoryOptions as mem (mem)}
                <OptionCard selected={memoryGb === mem} onclick={() => { memoryGb = mem; diskGb = 0; }}>
                  <strong>{mem} GB</strong>
                </OptionCard>
              {/each}
            </div>

          {:else if cstep === 5}
            <p class="hint">Storage is bundled with the plan — pick the disk size that fits.</p>
            <div class="option-grid">
              {#each storageOptions as s (s.slug)}
                <OptionCard selected={diskGb === s.diskGb} onclick={() => (diskGb = s.diskGb)}>
                  <strong>{s.diskGb} GB SSD</strong>
                  <span class="meta">${s.hourly.toFixed(3)}/hr · ${s.monthly}/mo</span>
                </OptionCard>
              {/each}
            </div>
            {#if chosenSize}<p class="meta">Plan: <code>{chosenSize.slug}</code></p>{/if}

          {:else if cstep === 6}
            <p class="hint">
              Additional block storage is a persistent volume — it survives Stop/Start
              cycles and costs ~$0.10/GB per month while it exists.
            </p>
            <div class="option-grid">
              {#each VOLUME_OPTIONS as gb (gb)}
                <OptionCard selected={volumeGb === gb} onclick={() => (volumeGb = gb)}>
                  <strong>{gb === 0 ? 'None' : `${gb} GB`}</strong>
                  {#if gb > 0}
                    <span class="meta">~${volumeHourly(gb).toFixed(4)}/hr · ${(gb * 720 * VOLUME_HOURLY_PER_GB).toFixed(2)}/mo</span>
                  {/if}
                </OptionCard>
              {/each}
            </div>

          {:else if cstep === 7}
            <p class="hint">
              Keys are injected into the droplet at boot. Select from your account, or
              paste a new public key to create one.
            </p>
            {#each options?.sshKeys ?? [] as key (key.id)}
              <label class="check-row">
                <input
                  type="checkbox"
                  checked={newSshIds.includes(key.id)}
                  onchange={() => {
                    newSshIds = newSshIds.includes(key.id)
                      ? newSshIds.filter((x) => x !== key.id)
                      : [...newSshIds, key.id];
                  }}
                />
                {key.name}
                <span class="meta">{key.fingerprint}</span>
              </label>
            {/each}
            {#if newSshIds.length === 0}<p class="hint">Select at least one key — otherwise you can't SSH in.</p>{/if}

            <div class="form-group" style="margin-top: 1rem;">
              <label for="nkey">Create a key</label>
              <p class="hint" style="margin-bottom: 0.5rem;">
                <strong>Generate</strong> creates an ed25519 pair on the dashboard server
                (private half stays at <code>data/ssh-keys/</code>), imports the public half
                to DigitalOcean, and selects it. <strong>Paste</strong> imports your own public key.
              </p>
              <input
                id="nkey"
                bind:value={newKeyPub}
                placeholder="paste a public key: ssh-ed25519 AAAA… user@host"
              />
              <div class="btn-row" style="margin-top: 0.5rem;">
                <input
                  style="max-width: 12rem;"
                  bind:value={newKeyName}
                  placeholder="name (optional — auto-generated)"
                  disabled={addingKey}
                />
                <button
                  class="btn small"
                  type="button"
                  onclick={() => addSshKey('generate')}
                  disabled={addingKey}
                >
                  {addingKey ? 'Generating…' : 'Generate on server'}
                </button>
                <button
                  class="btn small"
                  type="button"
                  onclick={() => addSshKey('paste')}
                  disabled={addingKey || !newKeyPub.trim()}
                >
                  {addingKey ? 'Adding…' : 'Import pasted key'}
                </button>
              </div>
              {#if keyStatus}<p class="hint" style="margin-top: 0.5rem;">{keyStatus}</p>{/if}
            </div>

          {:else if cstep === 8}
            <div class="form-group">
              <label for="mname">Name</label>
              <input id="mname" bind:value={newName} placeholder="big-build" />
            </div>
            <div class="form-group">
              <label for="midle">Idle auto-destroy (min, 0 = off)</label>
              <input id="midle" type="number" min="0" bind:value={newIdle} />
            </div>
            <div class="form-group">
              <label for="mrepo">Repository URL (optional)</label>
              <input id="mrepo" bind:value={newRepo} placeholder="https://github.com/owner/repo" />
            </div>

            <div class="review">
              <p><span>Region</span> <strong>{newRegion}</strong></p>
              <p><span>Image</span> <strong>{newSnapshot ? `snapshot ${newSnapshot}` : newImage || 'none yet'}</strong></p>
              <p><span>Plan</span> <strong>{chosenSize ? `${chosenSize.slug} — ${chosenSize.vcpus} vCPU · ${chosenSize.memoryGb} GB RAM · ${chosenSize.diskGb} GB SSD` : '—'}</strong></p>
              <p><span>Extra storage</span> <strong>{volumeGb > 0 ? `${volumeGb} GB volume` : 'none'}</strong></p>
              <p><span>SSH keys</span> <strong>{newSshIds.length} selected</strong></p>
              <p><span>Options</span> <strong>No backups · IPv4 only · No monitoring{dedicated ? ' · Dedicated VM' : ''}</strong></p>
              {#if chosenSize}
                <p>
                  <span>Cost</span>
                  <strong>${(chosenSize.hourly + volumeHourly(volumeGb)).toFixed(4)}/hr</strong>
                </p>
                <p class="meta" style="justify-content: flex-end;">
                  plan ${chosenSize.hourly.toFixed(4)}/hr{volumeGb > 0 ? ` + volume $${volumeHourly(volumeGb).toFixed(4)}/hr` : ''}
                </p>
              {/if}
            </div>
          {/if}
    {/if}
  </Wizard>
  </div>
{/if}

<ConfirmDialog
  bind:open={deleteOpen}
  title="Delete machine"
  message={deleteTarget
    ? `This destroys the droplet and volume on DigitalOcean and removes the profile “${deleteTarget.name}” (id ${deleteTarget.id}). This cannot be undone.`
    : ''}
  confirmLabel="Delete machine"
  danger
  requireText={deleteTarget?.name ?? ''}
  busy={deleteBusy}
  error={deleteError}
  onconfirm={doDelete}
/>
</AppShell>
