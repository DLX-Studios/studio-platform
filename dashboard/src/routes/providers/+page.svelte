<script lang="ts">
  import AppShell from '#lib/components/AppShell.svelte';
  import { Cloud } from 'phosphor-svelte';

  let { data } = $props();
</script>

<AppShell>
  <main class="container">
    <h1><Cloud size={24} weight="duotone" /> Providers</h1>
    <p class="hint">Cloud providers your builder machines can run on.</p>

    <div class="card machine-card">
      <div class="card-head" style="cursor: default;">
        <Cloud size={22} weight="duotone" />
        <strong>DigitalOcean</strong>
      </div>
      <p class="meta">Droplets · Spaces</p>
      <p class="meta">
        {data.doConnected
          ? '✅ Connected — token configured during setup.'
          : '❌ Not connected — run the setup wizard.'}
      </p>
    </div>

    <div class="card placeholder">
      <p class="hint">AWS, Hetzner, Vultr and Google Cloud support is coming.</p>
    </div>
  </main>
</AppShell>
