<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    selected?: boolean;
    disabled?: boolean;
    compact?: boolean;
    onclick?: () => void;
    badge?: { label: string; kind: 'rec' | 'soon' };
    children: Snippet;
  }

  let { selected = false, disabled = false, compact = false, onclick, badge, children }: Props = $props();
</script>

<button
  type="button"
  class="option-card"
  class:selected
  class:compact
  {disabled}
  onclick={onclick}
>
  {#if badge}
    <span class="p-badge {badge.kind}">{badge.label}</span>
  {/if}
  {@render children()}
</button>

<style>
  .option-card {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.3rem;
    padding: 0.85rem;
    border-radius: var(--radius-sm);
    border: var(--border-width) solid var(--border);
    background: var(--background);
    color: var(--foreground);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .option-card:hover:not(:disabled) {
    border-color: var(--primary);
  }

  .option-card.selected {
    border-color: var(--primary);
    background: color-mix(in srgb, var(--powder-blue) 10%, transparent);
  }

  .option-card:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  /* Compact variant — version chips inside grouped rows */
  .option-card.compact {
    padding: 0.45rem 0.7rem;
    align-items: center;
    flex-direction: row;
  }

  .option-card.compact :global(strong) {
    font-size: 0.85rem;
    padding-right: 0;
  }

  .option-card :global(strong) {
    font-size: 0.95rem;
    padding-right: 3.5rem; /* keep clear of the badge */
  }

  .option-card :global(.meta) {
    font-size: 0.75rem;
  }

  .option-card :global(.provider-icon) {
    color: var(--muted-foreground);
  }

  .option-card.selected :global(.provider-icon) {
    color: var(--primary);
  }

  .p-badge {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 0.15rem 0.45rem;
    border-radius: var(--radius-full);
    white-space: nowrap;
  }

  .p-badge.soon {
    background: color-mix(in srgb, var(--blue-slate) 25%, transparent);
    color: var(--muted-foreground);
  }

  .p-badge.rec {
    background: color-mix(in srgb, var(--powder-blue) 20%, transparent);
    color: var(--primary);
  }
</style>
