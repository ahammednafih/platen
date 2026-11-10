<script lang="ts">
  import DevicePanel from "./lib/DevicePanel.svelte";
  import ScanScreen from "./lib/ScanScreen.svelte";
  import ScanSettings from "./lib/ScanSettings.svelte";
  import Start from "./lib/Start.svelte";
  import { api, type SessionView } from "./lib/api";

  let connected = $state(false);
  let session = $state<SessionView | null>(null);

  async function close() {
    await api.closeSession();
    session = null;
  }
</script>

<div class="shell">
  <aside>
    <DevicePanel bind:connected />
    <ScanSettings />
  </aside>
  <main>
    {#if session}
      <ScanScreen bind:session {connected} onexport={() => {}} onclose={close} />
    {:else}
      <Start onopen={(s) => (session = s)} />
    {/if}
  </main>
</div>

<style>
  .shell { display: grid; grid-template-columns: 260px 1fr; height: 100%; }
  aside { border-right: 1px solid var(--line); padding: 20px 16px; display: grid; align-content: start; gap: 24px; overflow: auto; }
  main { overflow: auto; min-width: 0; }
</style>
