<script lang="ts">
  let {
    open = $bindable(false),
    title = 'Confirm',
    message = '',
    confirmLabel = 'Confirm',
    danger = false,
    /** Text the user must type exactly before the action unlocks. */
    requireText = '',
    busy = false,
    error = '',
    onconfirm,
  }: {
    open?: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    danger?: boolean;
    requireText?: string;
    busy?: boolean;
    error?: string;
    onconfirm: () => void;
  } = $props();

  let typed = $state('');

  // Reset the confirmation input each time the dialog opens.
  $effect(() => {
    if (open) typed = '';
  });

  const ready = $derived(!requireText || typed.trim() === requireText);

  function submit() {
    if (ready && !busy) onconfirm();
  }
</script>

{#if open}
  <div
    class="dialog-backdrop"
    role="presentation"
    onclick={(e) => e.target === e.currentTarget && !busy && (open = false)}
  >
    <div class="card dialog" role="dialog" aria-modal="true" aria-label={title}>
      <div class="dialog-head">
        <h2>{title}</h2>
      </div>
      {#if error}<div class="alert error">{error}</div>{/if}
      {#if message}<p class="hint" style="margin-bottom: 1rem;">{message}</p>{/if}

      {#if requireText}
        <div class="form-group">
          <label for="confirm-typed">
            Type <strong>{requireText}</strong> to confirm
          </label>
          <input
            id="confirm-typed"
            bind:value={typed}
            onkeydown={(e) => e.key === 'Enter' && submit()}
            disabled={busy}
          />
        </div>
      {/if}

      <div class="wizard-nav">
        <button class="btn ghost" onclick={() => (open = false)} disabled={busy}>Cancel</button>
        <button class="btn {danger ? 'danger' : 'primary'}" onclick={submit} disabled={!ready || busy}>
          {busy ? 'Working…' : confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
