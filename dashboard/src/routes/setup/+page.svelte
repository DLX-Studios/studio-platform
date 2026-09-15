<script lang="ts">
  import { goto } from '$app/navigation';
  import Wizard from '#lib/components/Wizard.svelte';
  import OptionCard from '#lib/components/OptionCard.svelte';
  import {
    CheckCircle,
    Cloud,
    CloudArrowUp,
    CloudFog,
    CloudLightning,
    CloudRain,
    Key,
    Palette,
    LockKey,
    GitBranch,
    HardDrives,
    Database,
    Sparkle,
    ChatCircleDots,
    TerminalWindow,
    Terminal,
    Brain,
    Lightning,
  } from 'phosphor-svelte';

  let step = $state(0);
  let busy = $state(false);
  let error = $state('');
  let hintOpen = $state('');

  const STEPS = ['Welcome', 'PIN', 'Provider', 'Repo', 'Storage', 'Agent', 'Defaults', 'Done'];

  // --- provider catalogs (only the supported entries are selectable) ---
  interface Provider {
    id: string;
    name: string;
    desc: string;
    icon: typeof Cloud;
    comingSoon: boolean;
  }

  const CLOUD_PROVIDERS: Provider[] = [
    { id: 'do', name: 'DigitalOcean', desc: 'Droplets · Spaces', icon: Cloud, comingSoon: false },
    { id: 'aws', name: 'AWS', desc: 'EC2 · Lightsail', icon: CloudArrowUp, comingSoon: true },
    { id: 'hetzner', name: 'Hetzner', desc: 'Cloud · Robot', icon: CloudLightning, comingSoon: true },
    { id: 'vultr', name: 'Vultr', desc: 'Compute', icon: CloudRain, comingSoon: true },
    { id: 'gcp', name: 'Google Cloud', desc: 'Compute Engine', icon: CloudFog, comingSoon: true },
  ];

  const STORAGE_PROVIDERS: Provider[] = [
    { id: 'spaces', name: 'DigitalOcean Spaces', desc: 'S3-compatible', icon: HardDrives, comingSoon: false },
    { id: 's3', name: 'AWS S3', desc: 'S3-compatible', icon: Database, comingSoon: true },
    { id: 'r2', name: 'Cloudflare R2', desc: 'S3-compatible · no egress fees', icon: Cloud, comingSoon: true },
    { id: 'b2', name: 'Backblaze B2', desc: 'S3-compatible', icon: HardDrives, comingSoon: true },
  ];

  const AGENT_PROVIDERS: Provider[] = [
    { id: 'openrouter', name: 'OpenRouter', desc: '400+ models · API key', icon: Sparkle, comingSoon: false },
    { id: 'claude', name: 'Claude', desc: 'Anthropic · API key', icon: ChatCircleDots, comingSoon: true },
    { id: 'claude-code', name: 'Claude Code', desc: 'Anthropic · CLI harness', icon: TerminalWindow, comingSoon: true },
    { id: 'codex', name: 'Codex', desc: 'OpenAI · CLI harness', icon: Terminal, comingSoon: true },
    { id: 'openai', name: 'OpenAI', desc: 'Direct API key', icon: Brain, comingSoon: true },
    { id: 'grok', name: 'Grok', desc: 'xAI · API key', icon: Lightning, comingSoon: true },
  ];

  let cloudProvider = $state('do');
  let storageProvider = $state('spaces');
  let agentProvider = $state('openrouter');

  let pin = $state('');
  let pin2 = $state('');
  let doToken = $state('');
  let probed = $state(false);
  let probing = $state(false);
  let sshKeys = $state<{ id: number; name: string; fingerprint: string }[]>([]);
  let snapshots = $state<{ id: string; name: string }[]>([]);
  let selectedKeyIds = $state<number[]>([]);
  let snapshotId = $state('');

  let repoUrl = $state('');
  let gitToken = $state('');

  let storageTab = $state<'auto' | 'manual'>('auto');
  let spacesBucket = $state('studio-cache');
  let spacesRegion = $state('nyc3');
  let spacesKeyId = $state('');
  let spacesSecret = $state('');
  let spacesChecking = $state(false);
  let spacesCreating = $state(false);
  let spacesProbed = $state(false);
  let spacesExists = $state(false);
  let spacesStatus = $state('');

  let openrouterKey = $state('');
  let region = $state('nyc3');
  let size = $state('c-8');

  const sizes = ['c-4', 'c-8', 'c-16', 'c-32'];
  const regions = ['nyc3', 'sfo3', 'ams3', 'lon1', 'fra1', 'tor1', 'syd1', 'sgp1', 'blr1'];

  // Step headings render in a fixed header above the scrollable content.
  const STEP_HEADINGS: { label: string; icon: typeof Cloud; weight: 'duotone' | 'fill' }[] = [
    { label: 'Welcome', icon: Cloud, weight: 'duotone' },
    { label: 'Choose a PIN', icon: LockKey, weight: 'duotone' },
    { label: 'Cloud provider', icon: Cloud, weight: 'duotone' },
    { label: 'Repository', icon: GitBranch, weight: 'duotone' },
    { label: 'Storage', icon: HardDrives, weight: 'duotone' },
    { label: 'AI agent', icon: Palette, weight: 'duotone' },
    { label: 'Defaults', icon: Key, weight: 'duotone' },
    { label: "You're set", icon: CheckCircle, weight: 'fill' },
  ];

  function toggleHint(id: string) {
    hintOpen = hintOpen === id ? '' : id;
  }

  function toggleKey(id: number) {
    selectedKeyIds = selectedKeyIds.includes(id)
      ? selectedKeyIds.filter((k) => k !== id)
      : [...selectedKeyIds, id];
  }

  async function spacesProbe(action: 'check' | 'create') {
    spacesChecking = action === 'check';
    spacesCreating = action === 'create';
    spacesStatus = '';
    try {
      const r = await fetch('/api/setup/spaces/probe', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          accessKey: spacesKeyId,
          secretKey: spacesSecret,
          region: spacesRegion,
          bucket: spacesBucket,
          action,
        }),
      });
      const data = await r.json();
      spacesProbed = data.ok === true;
      spacesExists = data.exists === true;
      spacesStatus = data.message ?? data.error;
      // The scan may find the bucket living in a different region.
      if (data.region && data.region !== spacesRegion) spacesRegion = data.region;
    } catch (e) {
      spacesProbed = false;
      spacesExists = false;
      spacesStatus = (e as Error).message;
    }
    spacesChecking = false;
    spacesCreating = false;
  }

  async function probe() {
    probing = true;
    error = '';
    probed = false;
    sshKeys = [];
    snapshots = [];
    selectedKeyIds = [];
    try {
      const r = await fetch('/api/setup/probe', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: doToken }),
      });
      const data = await r.json();
      if (!r.ok) throw new Error(data.error);
      sshKeys = data.keys ?? [];
      snapshots = data.snapshots ?? [];
      probed = true;
      if (sshKeys.length === 1) selectedKeyIds = [sshKeys[0].id];
    } catch (e) {
      error = (e as Error).message;
    }
    probing = false;
  }

  async function finish() {
    busy = true;
    error = '';
    try {
      const r = await fetch('/api/setup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          pin,
          doToken,
          sshKeyIds: selectedKeyIds.join(','),
          snapshotId: snapshotId || undefined,
          gitToken: gitToken || undefined,
          repoUrl,
          spacesBucket,
          spacesRegion,
          spacesEndpoint: `${spacesRegion}.digitaloceanspaces.com`,
          spacesKeyId,
          spacesSecret,
          openrouterKey: openrouterKey || undefined,
          region,
          size,
        }),
      });
      const data = await r.json();
      if (!r.ok) {
        error = data.error;
        if (data.field === 'doToken' || data.field === 'sshKeyIds') step = 2;
        else if (data.field === 'gitToken' || data.field === 'repoUrl') step = 3;
        else if (data.field === 'spacesKeyId') step = 4;
        else if (data.field === 'openrouterKey') step = 5;
        else if (data.field === 'pin') step = 1;
        busy = false;
        return;
      }
      step = 7;
    } catch (e) {
      error = (e as Error).message;
    }
    busy = false;
  }

  function next() {
    error = '';
    if (step === 1 && (pin.length < 4 || pin !== pin2)) {
      error = pin.length < 4 ? 'PIN must be at least 4 characters.' : 'PINs do not match.';
      return;
    }
    if (step === 2 && (!probed || selectedKeyIds.length === 0)) {
      error = probed ? 'Select at least one SSH key.' : 'Check the Personal Access Token first.';
      return;
    }
    if (step === 4) {
      const fieldsOk =
        spacesBucket.trim() && spacesKeyId.trim() && spacesSecret.trim();
      if (!fieldsOk) {
        error = 'Bucket, access key, and secret are required.';
        return;
      }
      if (storageTab === 'auto' && !(spacesProbed && spacesExists)) {
        error = 'Find or create the bucket first (or switch to the Manual tab).';
        return;
      }
    }
    step = Math.min(step + 1, 7);
  }

  // nav state for the Wizard shell
  let navLabel = $derived(
    step === 0 ? 'Get started' : step === 6 ? 'Finish setup' : step === 7 ? 'Go to machines' : 'Continue',
  );
  let navCanNext = $derived(
    !(step === 1 && (pin.length < 4 || pin !== pin2)) &&
      !(step === 2 && (!probed || selectedKeyIds.length === 0)) &&
      !(step === 4 &&
        ((!spacesBucket.trim() || !spacesKeyId.trim() || !spacesSecret.trim()) ||
          (storageTab === 'auto' && !(spacesProbed && spacesExists)))),
  );
  function navNext() {
    if (step === 6) return finish();
    if (step === 7) return goto('/machines');
    next();
  }
</script>

{#snippet providerCard(p: Provider, selectedId: string, onSelect: (id: string) => void, badge?: { label: string; kind: 'rec' | 'soon' })}
  {@const Icon = p.icon}
  <OptionCard selected={p.id === selectedId} disabled={p.comingSoon} onclick={() => onSelect(p.id)} {badge}>
    <span class="provider-icon"><Icon size={22} weight="duotone" /></span>
    <strong>{p.name}</strong>
    <span class="meta">{p.desc}</span>
  </OptionCard>
{/snippet}

{#snippet stepHeading(i: number)}
  {@const heading = STEP_HEADINGS[i]}
  {@const Icon = heading.icon}
  <h1><Icon size={28} weight={heading.weight} /> {heading.label}</h1>
{/snippet}

{#snippet wizardHeading()}
  {@render stepHeading(step)}
{/snippet}

<main class="center-screen wizard-screen">
  <Wizard
    title="Setup"
    steps={STEPS}
    bind:step
    {error}
    nextLabel={navLabel}
    busyLabel="Validating & saving…"
    {busy}
    canNext={navCanNext}
    onnext={navNext}
  >
    {#snippet heading()}
      {@render wizardHeading()}
    {/snippet}

    {#if step === 0}
          <p class="hint">
            This wizard configures Studio Builder once. Tokens stay in local SQLite
            and only leave this machine to call your cloud provider, your git host,
            object storage, AI providers, and your builders.
          </p>

        {:else if step === 1}
          <div class="form-group">
            <label for="pin">PIN (min 4 characters)</label>
            <input id="pin" type="password" bind:value={pin} />
          </div>
          <div class="form-group">
            <label for="pin2">Confirm PIN</label>
            <input id="pin2" type="password" bind:value={pin2} />
          </div>

        {:else if step === 2}
          <p class="hint">Where your builder machines run. More providers are on the way.</p>
          <div class="option-grid">
            {#each CLOUD_PROVIDERS as p (p.id)}
              {@render providerCard(p, cloudProvider, (id) => (cloudProvider = id), p.comingSoon ? { label: 'Soon', kind: 'soon' } : undefined)}
            {/each}
          </div>

          {#if cloudProvider === 'do'}
            <div class="form-group">
              <label for="dot">DigitalOcean Personal Access Token</label>
              <input id="dot" type="password" bind:value={doToken} placeholder="dop_v1_…" oninput={() => (probed = false)} />
              <button type="button" class="hint-toggle" onclick={() => toggleHint('do')}>Where do I get this? →</button>
              {#if hintOpen === 'do'}
                <ol class="hint-list">
                  <li>
                    Open
                    <a href="https://cloud.digitalocean.com/account/api/tokens" target="_blank" rel="noopener">
                      Manage Personal Access Tokens
                    </a>
                  </li>
                  <li>Generate a token named <code>studio-builder</code>. Use global full access (or a custom scope that can create and destroy Droplets). Copy it immediately — DigitalOcean shows it once, and scopes cannot be edited later.</li>
                  <li>Paste it here, then click Check token. If it fails, the token was rejected or the network is down — don’t generate a new one until you know which.</li>
                </ol>
              {/if}
            </div>
            <button class="btn ghost" onclick={probe} disabled={!doToken || probing}>
              {probing ? 'Checking…' : 'Check token'}
            </button>

            {#if probed}
              <div class="form-group" style="margin-top: 1rem;">
                <span class="field-label">SSH keys</span>
                {#if sshKeys.length === 0}
                  <p class="hint">No keys on this account.</p>
                  <button type="button" class="hint-toggle" onclick={() => toggleHint('ssh')}>How do I add one? →</button>
                  {#if hintOpen === 'ssh'}
                    <ol class="hint-list">
                      <li>
                        Open
                        <a href="https://cloud.digitalocean.com/account/security" target="_blank" rel="noopener">
                          Settings → Security
                        </a>
                      </li>
                      <li>Add your laptop’s public key (<code>cat ~/.ssh/id_ed25519.pub</code>).</li>
                      <li>Come back and click Check token again.</li>
                    </ol>
                  {/if}
                {:else}
                  {#each sshKeys as k (k.id)}
                    <label class="check-row">
                      <input type="checkbox" checked={selectedKeyIds.includes(k.id)} onchange={() => toggleKey(k.id)} />
                      {k.name} <span class="meta">({k.id})</span>
                    </label>
                  {/each}
                {/if}
              </div>
              <div class="form-group">
                <label for="snap">Golden snapshot</label>
                {#if snapshots.length === 0}
                  <p class="hint">No droplet snapshots yet — leave empty and set per-machine later, or take one from a prepared builder.</p>
                {:else}
                  <select id="snap" bind:value={snapshotId}>
                    <option value="">— none yet —</option>
                    {#each snapshots as s (s.id)}
                      <option value={s.id}>{s.name} ({s.id})</option>
                    {/each}
                  </select>
                {/if}
              </div>
            {/if}
          {/if}

        {:else if step === 3}
          <p class="hint">
            Optional. The builder runs <code>git clone &lt;url&gt; studio</code> at boot —
            you can add or change the repository later on the machine profile.
          </p>
          <div class="form-group">
            <label for="rurl">Repository URL</label>
            <input id="rurl" bind:value={repoUrl} placeholder="https://github.com/owner/repo" />
            <button type="button" class="hint-toggle" onclick={() => toggleHint('repo')}>Which URL works? →</button>
            {#if hintOpen === 'repo'}
              <ol class="hint-list">
                <li>Public repo: paste the clone URL — any host (GitHub, GitLab, Codeberg, self-hosted).</li>
                <li>Private repo: either add a token below (we embed it in the HTTPS clone URL), or skip it and set up a deploy key / SSH remote on the machine later.</li>
                <li>You don’t need an account or key just to clone — public repos clone anonymously.</li>
              </ol>
            {/if}
          </div>
          <div class="form-group">
            <label for="gtok">Token (optional, for private repos)</label>
            <input id="gtok" type="password" bind:value={gitToken} placeholder="ghp_… / glpat-… (optional)" />
            <button type="button" class="hint-toggle" onclick={() => toggleHint('tok')}>When do I need this? →</button>
            {#if hintOpen === 'tok'}
              <ol class="hint-list">
                <li>Only for private repos over HTTPS. Public repos: leave empty.</li>
                <li>
                  GitHub:
                  <a href="https://github.com/settings/personal-access-tokens" target="_blank" rel="noopener">fine-grained tokens</a>
                  — this repo only, <strong>Contents: Read</strong>.
                </li>
                <li>GitLab: Preferences → Access Tokens → <code>read_repository</code>. SSH URLs (<code>git@…</code>) ignore this field.</li>
              </ol>
            {/if}
          </div>

        {:else if step === 4}
          <p class="hint">
            Object storage for the compilation cache — it survives destroying machines.
            Region must match the bucket.
          </p>
          <div class="option-grid">
            {#each STORAGE_PROVIDERS as p (p.id)}
              {@render providerCard(
                p,
                storageProvider,
                (id) => (storageProvider = id),
                p.id === 'spaces' && cloudProvider === 'do'
                  ? { label: 'Recommended', kind: 'rec' }
                  : p.comingSoon
                    ? { label: 'Soon', kind: 'soon' }
                    : undefined,
              )}
            {/each}
          </div>

          {#if storageProvider === 'spaces'}
            <div class="tab-row" role="tablist">
              <button
                type="button"
                role="tab"
                aria-selected={storageTab === 'auto'}
                class:active={storageTab === 'auto'}
                onclick={() => (storageTab = 'auto')}
              >
                Automatic
              </button>
              <button
                type="button"
                role="tab"
                aria-selected={storageTab === 'manual'}
                class:active={storageTab === 'manual'}
                onclick={() => (storageTab = 'manual')}
              >
                Manual
              </button>
            </div>

            {#if storageTab === 'auto'}
              <p class="hint">
                Paste your Spaces keys and we’ll find your bucket — or create one.
                Keys are generated once in the
                <a href="https://cloud.digitalocean.com/account/api/spaces" target="_blank" rel="noopener">Spaces key manager</a>.
              </p>
              <div class="form-row">
                <div class="form-group">
                  <label for="skid">Access key</label>
                  <input id="skid" bind:value={spacesKeyId} oninput={() => (spacesProbed = false)} />
                </div>
                <div class="form-group">
                  <label for="ssec">Secret key</label>
                  <input id="ssec" type="password" bind:value={spacesSecret} oninput={() => (spacesProbed = false)} />
                </div>
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label for="sbu">Bucket name</label>
                  <input id="sbu" bind:value={spacesBucket} oninput={() => (spacesProbed = false)} />
                </div>
                <div class="form-group">
                  <label for="sreg">Region</label>
                  <select id="sreg" bind:value={spacesRegion} onchange={() => (spacesProbed = false)}>
                    {#each regions as r (r)}<option value={r}>{r}</option>{/each}
                  </select>
                </div>
              </div>
              <div class="btn-row">
                <button
                  class="btn ghost"
                  onclick={() => spacesProbe('check')}
                  disabled={spacesChecking || spacesCreating || !spacesKeyId.trim() || !spacesSecret.trim() || !spacesBucket.trim()}
                >
                  {spacesChecking ? 'Checking…' : 'Find bucket'}
                </button>
                {#if spacesProbed && !spacesExists}
                  <button
                    class="btn primary"
                    onclick={() => spacesProbe('create')}
                    disabled={spacesChecking || spacesCreating}
                  >
                    {spacesCreating ? 'Creating…' : `Create “${spacesBucket}” in ${spacesRegion}`}
                  </button>
                {/if}
              </div>
              {#if spacesStatus}<p class="hint" style="margin-top: 0.5rem;">{spacesStatus}</p>{/if}
            {:else}
              <div class="form-group">
                <label for="sbu">Bucket name</label>
                <input id="sbu" bind:value={spacesBucket} />
              </div>
              <div class="form-row">
                <div class="form-group">
                  <label for="sreg">Region</label>
                  <select id="sreg" bind:value={spacesRegion}>
                    {#each regions as r (r)}<option value={r}>{r}</option>{/each}
                  </select>
                </div>
                <div class="form-group">
                  <label for="skid">Access key</label>
                  <input id="skid" bind:value={spacesKeyId} />
                </div>
              </div>
              <div class="form-group">
                <label for="ssec">Secret key</label>
                <input id="ssec" type="password" bind:value={spacesSecret} />
              </div>
            {/if}
          {/if}

        {:else if step === 5}
          <p class="hint">
            The on-machine coding agent. Optional — you can add a provider later in Settings.
          </p>
          <div class="option-grid">
            {#each AGENT_PROVIDERS as p (p.id)}
              {@render providerCard(
                p,
                agentProvider,
                (id) => (agentProvider = id),
                p.id === 'openrouter'
                  ? { label: 'Recommended', kind: 'rec' }
                  : { label: 'Soon', kind: 'soon' },
              )}
            {/each}
          </div>

          {#if agentProvider === 'openrouter'}
            <div class="form-group">
              <label for="ork">OpenRouter API key</label>
              <input id="ork" type="password" bind:value={openrouterKey} placeholder="sk-or-… (optional)" />
              <button type="button" class="hint-toggle" onclick={() => toggleHint('or')}>Where do I get this? →</button>
              {#if hintOpen === 'or'}
                <ol class="hint-list">
                  <li>
                    Open
                    <a href="https://openrouter.ai/settings/keys" target="_blank" rel="noopener">
                      OpenRouter → Keys
                    </a>
                  </li>
                  <li>Create a key. Default scope is enough.</li>
                  <li>Paste it here, or skip — the rest of the app works without it.</li>
                </ol>
              {/if}
            </div>
          {/if}

        {:else if step === 6}
          <p class="hint">Used for new machines; each profile can override.</p>
          <div class="form-row">
            <div class="form-group">
              <label for="reg">Region</label>
              <select id="reg" bind:value={region}>
                {#each regions as r (r)}<option value={r}>{r}</option>{/each}
              </select>
            </div>
            <div class="form-group">
              <label for="sz">Default size</label>
              <select id="sz" bind:value={size}>
                {#each sizes as s (s)}<option value={s}>{s}</option>{/each}
              </select>
            </div>
          </div>

        {:else}
          <p class="hint">
            Next: create a machine profile. If you skipped a snapshot, set one on the
            profile after you snapshot a prepared builder.
          </p>
        {/if}
  </Wizard>
</main>
