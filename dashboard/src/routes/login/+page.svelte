<script lang="ts">
  import { goto } from '$app/navigation';
  import { LockKey } from 'phosphor-svelte';

  let pin = $state('');
  let error = $state('');
  let busy = $state(false);
  let canSubmit = $derived(!busy && pin.length >= 4);

  async function submit() {
    if (!canSubmit) return;

    busy = true;
    error = '';
    try {
      const r = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin }),
      });
      const data = (await r.json().catch(() => ({}))) as { error?: string };
      if (!r.ok) throw new Error(data.error ?? `Login failed (${r.status}). Please try again.`);
      await goto('/machines');
    } catch (e) {
      error = e instanceof Error ? e.message : 'Login failed. Please try again.';
    } finally {
      busy = false;
    }
  }
</script>

<main class="center-screen">
  <form class="card auth-card" onsubmit={(e) => { e.preventDefault(); submit(); }}>
    <LockKey size={28} weight="duotone" />
    <h1>Studio Builder</h1>
    {#if error}<div class="alert error">{error}</div>{/if}
    <div class="form-group">
      <label for="pin">PIN</label>
      <input
        id="pin"
        type="password"
        inputmode="numeric"
        autocomplete="current-password"
        bind:value={pin}
        placeholder="••••"
      />
    </div>
    <button class="btn primary" type="submit" disabled={!canSubmit} aria-busy={busy}>
      {busy ? 'Checking…' : 'Unlock'}
    </button>
  </form>
</main>
