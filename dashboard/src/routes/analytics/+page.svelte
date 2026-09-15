<script lang="ts">
  import { fade } from 'svelte/transition';
  import { ChartLine, CaretLeft, Cloud } from 'phosphor-svelte';

  let { data } = $props();
</script>

  <h1>
    <ChartLine size={28} />
    Analytics
  </h1>

  <div class="stats">
      <div class="stat" transition:fade={{ duration: 0.3 }}>
        <span>{data.totalHours}</span>
        <span class="stat-label">Hours Used</span>
      </div>
      <div class="stat" transition:fade={{ duration: 0.3 }}>
        <span>${data.totalCost}</span>
        <span class="stat-label">Est. Cost</span>
      </div>
      <div class="stat" transition:fade={{ duration: 0.3 }}>
        <span>{data.sessions.length}</span>
        <span class="stat-label">Sessions</span>
      </div>
  </div>

  <div class="card" style="padding:0; overflow:hidden;">
    <table aria-label="Build session history">
      <thead>
        <tr>
          <th>Started</th>
          <th>Size</th>
          <th>Hours</th>
          <th class="text-right">Est. Cost</th>
        </tr>
      </thead>
      <tbody>
        {#each data.sessions as s}
          <tr>
            <td>{new Date(s.started_at).toLocaleString()}</td>
            <td>
              <span class="size-badge">{s.size}</span>
            </td>
            <td>{s.hours}</td>
            <td class="text-right">${s.cost}</td>
          </tr>
        {:else}
          <tr>
            <td colspan="4" style="text-align:center; opacity:0.7;">No sessions yet.</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <nav>
    <a href="/">
      <CaretLeft size={14} />
      Dashboard
    </a>
  </nav>
