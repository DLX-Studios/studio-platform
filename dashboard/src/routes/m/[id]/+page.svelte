<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { invalidate } from '$app/navigation';
  import {
    ArrowLeft,
    List,
    TerminalWindow,
    Files,
    ChartLine,
    GearSix,
    Scroll,
    Play,
    Square,
    Camera,
  } from 'phosphor-svelte';
  import AgentChat from '#lib/components/AgentChat.svelte';

  const id = $derived(Number(page.params.id));

  interface Machine {
    id: number; name: string; size: string; region: string;
    idle_timeout_min: number; repo: string | null; daemon_ready: number;
  }
  interface Status { running: boolean; state: string; ip: string | null }

  let { data } = $props();

  let machine = $derived(data.machine);
  let status = $derived(data.status);
  let busy = $state(false);
  let poll: ReturnType<typeof setInterval>;

  // shell state
  let sidebarOpen = $state(true);
  let chatTab = $state<'chat' | 'console'>('chat');
  let mainMode = $state<'view' | 'files' | 'logs' | 'analytics' | 'settings'>('view');
  let chatWidth = $state(420); // px, min 320

  let opError = $state('');

  async function act(action: 'start' | 'stop' | 'snapshot') {
    busy = true;
    opError = '';
    try {
      const r = await fetch(`/api/machines/${id}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      if (!r.ok) {
        const data = await r.json().catch(() => ({}) as { error?: string });
        opError = data.error ?? `Request failed (${r.status}).`;
      }
      await invalidate(`app:machine:${id}`);
    } finally {
      busy = false;
    }
  }

  function dotClass(): string {
    if (!status.running) return 'dot off';
    if (status.state === 'starting') return 'dot winding';
    if (status.state === 'provisioning') return 'dot provisioning';
    return 'dot ready';
  }

  // Daemon-confirmed readiness gates the live panels.
  const booting = $derived(status.state === 'starting' || status.state === 'provisioning');
  const live = $derived(status.state === 'ready');

  const modes = [
    { key: 'view', icon: TerminalWindow, label: 'Remote view' },
    { key: 'files', icon: Files, label: 'Files' },
    { key: 'logs', icon: Scroll, label: 'Logs' },
    { key: 'analytics', icon: ChartLine, label: 'Analytics' },
    { key: 'settings', icon: GearSix, label: 'Settings' },
  ] as const;

  /* console resizer */
  let dragging = $state(false);
  function startDrag(e: MouseEvent) {
    dragging = true;
    e.preventDefault();
  }
  function onMove(e: MouseEvent) {
    if (!dragging) return;
    chatWidth = Math.min(Math.max(e.clientX, 320), window.innerWidth - 400);
  }
  function stopDrag() { dragging = false; }

  onMount(() => {
    poll = setInterval(() => invalidate(`app:machine:${id}`), 5000);
    return () => clearInterval(poll);
  });
</script>

<svelte:window onmousemove={onMove} onmouseup={stopDrag} />

<div class="m-shell" class:dragging>
  <header class="topbar m-topbar">
    <div class="tb-left">
      <a href="/machines" class="icon-btn" aria-label="Back to machines"><ArrowLeft size={18} /></a>
      <button class="icon-btn" onclick={() => (sidebarOpen = !sidebarOpen)} aria-label="Toggle sidebar">
        <List size={18} />
      </button>
    </div>
    <div class="tb-center">
      <span class={dotClass()}></span>
      <strong>{machine?.name ?? '…'}</strong>
      <span class="meta">{status.state}</span>
    </div>
    <div class="tb-right">
      {#if status.running}
        <button class="btn small" onclick={() => act('stop')} disabled={busy}>
          <Square size={12} weight="fill" /> Stop
        </button>
      {:else}
        <button class="btn small primary" onclick={() => act('start')} disabled={busy}>
          <Play size={12} weight="fill" /> Start
        </button>
      {/if}
      {#if status.running}
        <button class="btn small" onclick={() => act('snapshot')} disabled={busy} title="Take a golden snapshot of this machine">
          <Camera size={12} /> Snapshot
        </button>
      {/if}
      {#each modes as m}
        <button
          class="icon-btn"
          class:active={mainMode === m.key}
          onclick={() => (mainMode = m.key)}
          aria-label={m.label}
          title={m.label}
        >
          <m.icon size={18} />
        </button>
      {/each}
    </div>
  </header>

  {#if opError}
    <div class="alert error" style="margin: 0.75rem 1rem 0;">{opError}</div>
  {/if}

  <div class="m-body">
    {#if sidebarOpen}
      <aside class="chat-sidebar" style="width: {chatWidth}px; min-width: 20rem;">
        <div class="chat-tabs">
          <button class:active={chatTab === 'chat'} onclick={() => (chatTab = 'chat')}>Chat</button>
          <button class:active={chatTab === 'console'} onclick={() => (chatTab = 'console')}>Console</button>
        </div>

        {#if booting}
          <div class="chat-body placeholder">
            <div class="skeleton" style="height: 1rem; width: 65%;"></div>
            <div class="skeleton" style="height: 3.5rem;"></div>
            <div class="skeleton" style="height: 1rem; width: 45%;"></div>
            <p class="hint">
              {status.state === 'starting' ? 'Creating the droplet…' : 'Provisioning — waiting for the daemon to dial home…'}
            </p>
          </div>
        {:else if chatTab === 'chat'}
          {#key live}
            <AgentChat machineId={id} machineName={machine?.name ?? 'builder'} enabled={live} />
          {/key}
        {:else}
          <div class="chat-body console placeholder">
            <p class="hint">Console (PTY over daemon tunnel) lands in Phase 2.</p>
            {#if status.ip}<p class="hint">Meanwhile: <code>ssh root@{status.ip}</code></p>{/if}
          </div>
        {/if}
      </aside>
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="resizer"
        role="separator"
        aria-orientation="vertical"
        onmousedown={startDrag}
      ></div>
    {/if}

    <section class="main-area">
      {#if mainMode === 'view'}
        {#if booting}
          <div class="webrtc-stage placeholder">
            <div class="skeleton" style="height: 2rem; width: 50%;"></div>
            <div class="skeleton" style="height: 55%;"></div>
            <p class="hint">
              {status.state === 'starting' ? 'Creating the droplet…' : 'Provisioning — the stream appears once the daemon confirms the machine is ready.'}
            </p>
          </div>
        {:else}
          <div class="webrtc-stage placeholder">
            <p class="hint">
              {#if live}
                WebRTC app stream lands in Phase 3 (cage + screencopy + WHEP).
              {:else}
                Machine is off — press Start, then the stream appears here.
              {/if}
            </p>
            {#if status.ip}<p class="hint">IP: <code>{status.ip}</code></p>{/if}
          </div>
        {/if}
      {:else if mainMode === 'settings' && machine}
        <div class="card panel">
          <h2>Machine settings</h2>
          <p class="meta">Editing lands with the settings API (Phase 2). Current values:</p>
          <ul>
            <li>Name: <strong>{machine.name}</strong></li>
            <li>Size: {machine.size} · Region: {machine.region}</li>
            <li>Idle auto-destroy: {machine.idle_timeout_min === 0 ? 'off' : `${machine.idle_timeout_min} min`}</li>
            {#if machine.repo}<li>Repo override: {machine.repo}</li>{/if}
            <li>Daemon: {machine.daemon_ready ? 'connected' : 'not connected'}</li>
          </ul>
        </div>
      {:else if booting}
        <div class="card panel placeholder">
          <h2>{modes.find((m) => m.key === mainMode)?.label}</h2>
          <div class="skeleton" style="height: 1rem; width: 60%;"></div>
          <div class="skeleton" style="height: 1rem; width: 40%;"></div>
          <div class="skeleton" style="height: 6rem;"></div>
          <p class="hint">
            {status.state === 'starting' ? 'Creating the droplet…' : 'Provisioning — this panel unlocks once the daemon confirms the machine is ready.'}
          </p>
        </div>
      {:else}
        <div class="card panel placeholder">
          <h2>{modes.find((m) => m.key === mainMode)?.label}</h2>
          <p class="hint">This panel is wired to the daemon in Phase 2/3.</p>
        </div>
      {/if}
    </section>
  </div>
</div>
