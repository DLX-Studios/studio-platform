<script lang="ts">
  import { goto, afterNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import type { Snippet } from 'svelte';
  import {
    List,
    Cloud,
    Plus,
    CaretDown,
    UserCircle,
    SignOut,
    HardDrives,
    Key,
    Sparkle,
    CheckCircle,
  } from 'phosphor-svelte';
  import OptionCard from './OptionCard.svelte';

  let { children, onnew }: { children: Snippet; onnew?: (kind: 'machine') => void } = $props();

  let sidebarOpen = $state(true);
  let newMenuOpen = $state(false);
  let avatarMenuOpen = $state(false);

  const NAV = [
    { label: 'Machines', href: '/machines', icon: HardDrives },
    { label: 'SSH Keys', href: '/ssh-keys', icon: Key },
    { label: 'Providers', href: '/providers', icon: Cloud },
    { label: 'AI Agents', href: '/agents', icon: Sparkle },
  ];

  const CLOUD_PROVIDERS = [
    { id: 'do', name: 'DigitalOcean', desc: 'Droplets · Spaces' },
    { id: 'aws', name: 'AWS', desc: 'EC2 · Lightsail' },
    { id: 'hetzner', name: 'Hetzner', desc: 'Cloud · Robot' },
    { id: 'vultr', name: 'Vultr', desc: 'Compute' },
    { id: 'gcp', name: 'Google Cloud', desc: 'Compute Engine' },
  ];

  const AGENT_PROVIDERS = [
    { id: 'openrouter', name: 'OpenRouter', desc: '400+ models · API key' },
    { id: 'claude', name: 'Claude', desc: 'Anthropic · API key' },
    { id: 'claude-code', name: 'Claude Code', desc: 'Anthropic · CLI harness' },
    { id: 'codex', name: 'Codex', desc: 'OpenAI · CLI harness' },
    { id: 'openai', name: 'OpenAI', desc: 'Direct API key' },
    { id: 'grok', name: 'Grok', desc: 'xAI · API key' },
  ];

  let providerDialogOpen = $state(false);
  let agentDialogOpen = $state(false);
  let openrouterConfigured = $state(false);
  let optionsLoading = $state(false);

  afterNavigate(() => {
    // Close the sidebar after in-app navigation on mobile so it doesn't stay open.
    if (window.matchMedia('(max-width: 48rem)').matches) sidebarOpen = false;
    newMenuOpen = false;
    avatarMenuOpen = false;
  });

  function onNewSelect(kind: 'machine' | 'provider' | 'agent') {
    newMenuOpen = false;
    if (kind === 'machine') {
      onnew?.('machine');
      return;
    }
    if (kind === 'provider') {
      providerDialogOpen = true;
    } else {
      agentDialogOpen = true;
    }
    if (!optionsLoading && !openrouterState.loaded) void loadIntegrations();
  }

  let openrouterState = $state({ loaded: false });

  async function loadIntegrations() {
    optionsLoading = true;
    try {
      const r = await fetch('/api/do/options');
      const ct = r.headers.get('content-type') ?? '';
      if (r.ok && ct.includes('application/json')) {
        const data = await r.json().catch(() => null);
        openrouterConfigured = !!data?.openrouter_configured;
        openrouterState.loaded = true;
      }
    } catch {
      /* dialog still renders with unknown state */
    }
    optionsLoading = false;
  }

  async function logout() {
    avatarMenuOpen = false;
    await fetch('/api/auth/logout', { method: 'POST' });
    goto('/login');
  }
</script>

<svelte:window
  onclick={(e) => {
    const t = e.target as HTMLElement;
    if (newMenuOpen && !t.closest('.menu-anchor-new')) newMenuOpen = false;
    if (avatarMenuOpen && !t.closest('.menu-anchor-avatar')) avatarMenuOpen = false;
  }}
/>

<div class="app-shell">
  <header class="topbar app-topbar">
    <div class="tb-group">
      <button class="icon-btn" onclick={() => (sidebarOpen = !sidebarOpen)} aria-label="Toggle sidebar">
        <List size={18} />
      </button>
      <a href="/machines" class="brand"><Cloud size={20} weight="duotone" /> Studio Builder</a>
    </div>

    <div class="tb-group">
      <div class="menu-anchor-new" style="position: relative;">
        <button
          class="btn small primary"
          onclick={() => {
            newMenuOpen = !newMenuOpen;
            avatarMenuOpen = false;
          }}
          aria-haspopup="menu"
          aria-expanded={newMenuOpen}
        >
          <Plus size={14} weight="bold" /> New <CaretDown size={12} />
        </button>
        {#if newMenuOpen}
          <div class="menu" role="menu">
            <button type="button" role="menuitem" onclick={() => onNewSelect('machine')}>
              <HardDrives size={16} /> Machine
            </button>
            <button type="button" role="menuitem" onclick={() => onNewSelect('provider')}>
              <Cloud size={16} /> Provider
            </button>
            <button type="button" role="menuitem" onclick={() => onNewSelect('agent')}>
              <Sparkle size={16} /> AI Agent
            </button>
          </div>
        {/if}
      </div>

      <div class="menu-anchor-avatar" style="position: relative;">
        <button
          class="avatar-btn"
          onclick={() => {
            avatarMenuOpen = !avatarMenuOpen;
            newMenuOpen = false;
          }}
          aria-haspopup="menu"
          aria-expanded={avatarMenuOpen}
          aria-label="Account"
        >
          <UserCircle size={28} weight="duotone" />
        </button>
        {#if avatarMenuOpen}
          <div class="menu" role="menu">
            <button type="button" role="menuitem" onclick={logout}>
              <SignOut size={16} /> Log out
            </button>
          </div>
        {/if}
      </div>
    </div>
  </header>

  <div class="shell-body">
    <aside class="sidebar" class:open={sidebarOpen}>
      <nav aria-label="Main">
        {#each NAV as item (item.href)}
          {@const Icon = item.icon}
          <a
            href={item.href}
            class="sidebar-item"
            class:active={page.url.pathname === item.href}
          >
            <Icon size={18} /> {item.label}
          </a>
        {/each}
      </nav>
    </aside>

    <main class="shell-main">
      {@render children()}
    </main>
  </div>

  <footer class="footer"><span>build machines, by the hour</span></footer>
</div>

{#if providerDialogOpen}
  <div
    class="dialog-backdrop"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && (providerDialogOpen = false)}
  >
    <div class="card dialog" role="dialog" aria-modal="true" aria-label="Providers">
      <div class="dialog-head">
        <h2>Providers</h2>
        <button class="btn small" onclick={() => (providerDialogOpen = false)} aria-label="Close">
          <CaretDown size={14} style="rotate: -90deg;" />
        </button>
      </div>
      <p class="hint">Where your builder machines run. Manage the connected provider here — more are coming.</p>
      <div class="option-grid">
        {#each CLOUD_PROVIDERS as p (p.id)}
          <OptionCard
            selected={p.id === 'do'}
            disabled={p.id !== 'do'}
            badge={p.id === 'do' ? { label: 'Connected', kind: 'rec' } : { label: 'Soon', kind: 'soon' }}
          >
            <strong>{p.name}</strong>
            <span class="meta">{p.desc}</span>
          </OptionCard>
        {/each}
      </div>
      <p class="hint">DigitalOcean is configured with your account token from setup.</p>
    </div>
  </div>
{/if}

{#if agentDialogOpen}
  <div
    class="dialog-backdrop"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && (agentDialogOpen = false)}
  >
    <div class="card dialog" role="dialog" aria-modal="true" aria-label="AI agents">
      <div class="dialog-head">
        <h2>AI agents</h2>
        <button class="btn small" onclick={() => (agentDialogOpen = false)} aria-label="Close">
          <CaretDown size={14} style="rotate: -90deg;" />
        </button>
      </div>
      <p class="hint">The on-machine coding agent provider. Manage the connected one here — more are coming.</p>
      <div class="option-grid">
        {#each AGENT_PROVIDERS as p (p.id)}
          <OptionCard
            selected={p.id === 'openrouter' && openrouterConfigured}
            disabled={p.id !== 'openrouter'}
            badge={
              p.id === 'openrouter'
                ? openrouterConfigured
                  ? { label: 'Connected', kind: 'rec' }
                  : { label: 'Recommended', kind: 'rec' }
                : { label: 'Soon', kind: 'soon' }
            }
          >
            <strong>{p.name}</strong>
            <span class="meta">{p.desc}</span>
          </OptionCard>
        {/each}
      </div>
      <p class="hint">
        {openrouterConfigured
          ? 'OpenRouter is configured with the API key from setup.'
          : 'OpenRouter is not configured yet — add an API key via the setup wizard.'}
      </p>
    </div>
  </div>
{/if}
