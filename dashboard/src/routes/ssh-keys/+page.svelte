<script lang="ts">
  import AppShell from '#lib/components/AppShell.svelte';
  import { Key, Plus } from 'phosphor-svelte';

  let { data } = $props();
</script>

<AppShell>
  <main class="container">
    <h1><Key size={24} weight="duotone" /> SSH keys</h1>
    <p class="hint">
      Keys on your DigitalOcean account. New machines embed the keys you select
      in the creation wizard; you can also paste or generate keys there.
    </p>

    {#if data.error}
      <div class="alert error">{data.error}</div>
    {:else if data.keys.length === 0}
      <div class="card empty-state">
        <Key size={36} weight="duotone" />
        <p>No SSH keys on this account yet.</p>
        <p class="hint">Create one from the New → Machine wizard's SSH keys step.</p>
      </div>
    {:else}
      <div class="card">
        {#each data.keys as k (k.id)}
          <div class="key-row">
            <strong>{k.name}</strong>
            <span class="meta">{k.fingerprint}</span>
          </div>
        {/each}
      </div>
    {/if}
  </main>
</AppShell>
