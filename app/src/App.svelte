<script lang="ts">
  import DevicePanel from "./lib/DevicePanel.svelte";
  import ExportDialog from "./lib/ExportDialog.svelte";
  import Library from "./lib/Library.svelte";
  import ScanScreen from "./lib/ScanScreen.svelte";
  import ScanSettings from "./lib/ScanSettings.svelte";
  import Start from "./lib/Start.svelte";
  import { api, type SessionView } from "./lib/api";

  let connected = $state(false);
  let session = $state<SessionView | null>(null);
  let exporting = $state(false);
  let view = $state<"scan" | "library">("scan");

  async function close() {
    await api.closeSession();
    session = null;
  }
</script>

<div class="shell">
  <aside>
    <nav>
      <button class:on={view === "scan"} onclick={() => (view = "scan")}>Scan</button>
      <button class:on={view === "library"} onclick={() => (view = "library")}>Saved scans</button>
    </nav>
    <DevicePanel bind:connected />
    {#if view === "scan"}
      <ScanSettings {connected} />
    {/if}
  </aside>
  <main>
    {#if view === "library"}
      <Library />
    {:else if session}
      <ScanScreen bind:session {connected} onexport={() => (exporting = true)} onclose={close} />
    {:else}
      <Start onopen={(s) => (session = s)} />
    {/if}
  </main>
</div>

{#if exporting && session}
  <ExportDialog
    pages={session.pages.length}
    onclose={() => (exporting = false)}
    onended={() => { exporting = false; session = null; }}
  />
{/if}

<style>
  .shell { display: grid; grid-template-columns: 260px 1fr; height: 100%; }
  aside { border-right: 1px solid var(--line); padding: 20px 16px; display: grid; align-content: start; gap: 24px; overflow: auto; }
  main { overflow: auto; min-width: 0; }
  nav { display: grid; gap: 2px; margin: -6px -6px 0; }
  nav button { text-align: left; border: none; background: none; padding: 7px 10px; font-weight: 500; }
  nav button.on { background: var(--sunken); font-weight: 600; }
</style>
