<script lang="ts">
  import DevicePanel from "./lib/DevicePanel.svelte";
  import Start from "./lib/Start.svelte";
  import type { SessionView } from "./lib/api";

  let connected = $state(false);
  let session = $state<SessionView | null>(null);
</script>

<div class="shell">
  <aside>
    <DevicePanel bind:connected />
  </aside>
  <main>
    {#if session}
      <p class="muted pad">Session {session.name} — {session.pages.length} pages</p>
    {:else}
      <Start onopen={(s) => (session = s)} />
    {/if}
  </main>
</div>

<style>
  .shell { display: grid; grid-template-columns: 260px 1fr; height: 100%; }
  aside { border-right: 1px solid var(--line); padding: 20px 16px; display: grid; align-content: start; gap: 24px; overflow: auto; }
  main { overflow: auto; min-width: 0; }
  .pad { padding: 24px; }
</style>
