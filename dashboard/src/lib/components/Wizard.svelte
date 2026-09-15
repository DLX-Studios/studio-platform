<script lang="ts">
  import type { Snippet } from 'svelte';
  import { ArrowLeft, ArrowRight, Check, X } from 'phosphor-svelte';

  interface Props {
    /** Rail heading, e.g. "Setup" or "New machine". */
    title: string;
    /** Step labels, rendered as the left-rail timeline. */
    steps: string[];
    /** Active step index — bindable, rail items jump back when clicked. */
    step?: number;
    /** Fixed heading above the scroll area (icon + step title). */
    heading?: Snippet;
    /** Scrollable step content. */
    children: Snippet;
    /** Error shown pinned above the heading. */
    error?: string;
    /** Right-hand nav button label. Omit `onnext` to hide the button. */
    nextLabel?: string;
    /** Label while `busy`. */
    busyLabel?: string;
    busy?: boolean;
    canNext?: boolean;
    onnext?: () => void;
    /** Renders an X close button (dialog variant). */
    onclose?: () => void;
    /** "page" = standalone centered card; "dialog" = inside a backdrop. */
    variant?: 'page' | 'dialog';
  }

  let {
    title,
    steps,
    step = $bindable(0),
    heading,
    children,
    error = '',
    nextLabel = 'Continue',
    busyLabel = 'Working…',
    busy = false,
    canNext = true,
    onnext,
    onclose,
    variant = 'page',
  }: Props = $props();
</script>

<div class="card wizard" class:as-dialog={variant === 'dialog'}>
  {#if onclose}
    <button class="wizard-close icon-btn" onclick={onclose} aria-label="Close"><X size={14} /></button>
  {/if}

  <aside class="wizard-rail" aria-label="{title} steps">
    <div class="rail-title">{title}</div>
    {#each steps as label, i (label)}
      <button
        type="button"
        class="rail-step"
        class:active={i === step}
        class:done={i < step}
        disabled={i > step}
        onclick={() => (step = i)}
      >
        <span class="rail-index">
          {#if i < step}<Check size={11} weight="bold" />{:else}{i + 1}{/if}
        </span>
        {label}
      </button>
    {/each}
  </aside>

  <section class="wizard-body">
    {#if error}<div class="alert error">{error}</div>{/if}

    {#if heading}{@render heading()}{/if}

    <div class="wizard-content">
      {@render children()}
    </div>

    <div class="wizard-nav">
      {#if step > 0}
        <button class="btn ghost" onclick={() => (step = Math.max(0, step - 1))}>
          <ArrowLeft size={16} /> Back
        </button>
      {:else}
        <span></span>
      {/if}
      {#if onnext}
        <button class="btn primary" onclick={onnext} disabled={busy || !canNext}>
          {busy ? busyLabel : nextLabel} <ArrowRight size={16} />
        </button>
      {/if}
    </div>
  </section>
</div>
