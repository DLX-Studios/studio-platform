<script lang="ts">
  import AppShell from '#lib/components/AppShell.svelte';
  import { Sparkle } from 'phosphor-svelte';

  let { data } = $props();
</script>

<AppShell>
  <main class="container">
    <h1><Sparkle size={24} weight="duotone" /> AI agents</h1>
    <p class="hint">The coding agent provider used on your builder machines.</p>

    <div class="card machine-card">
      <div class="card-head" style="cursor: default;">
        <Sparkle size={22} weight="duotone" />
        <strong>OpenRouter</strong>
      </div>
      <p class="meta">400+ models · API key</p>
      <p class="meta">
        {data.openrouter
          ? '✅ Connected — API key configured during setup.'
          : '❌ Not configured — add a key via the setup wizard.'}
      </p>
    </div>

    <div class="card placeholder">
      <p class="hint">
        Claude, Claude Code, Codex, OpenAI and Grok are coming — they will appear
        as selectable options in the machine wizard as they land.
      </p>
    </div>
  </main>
</AppShell>
