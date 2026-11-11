<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Device, type DeviceStatus } from "./api";
  import DeviceDetails from "./DeviceDetails.svelte";

  let { connected = $bindable(false) }: { connected?: boolean } = $props();

  let device = $state<Device | null>(null);
  let status = $state<DeviceStatus | null>(null);
  let error = $state("");
  let busy = $state(false);
  let host = $state("");
  let editing = $state(false);
  let details = $state(false);

  async function connect(h?: string) {
    busy = true;
    error = "";
    try {
      device = await api.connect(h || undefined);
      connected = true;
      editing = false;
      await refresh();
    } catch (e) {
      error = String(e);
      connected = false;
    } finally {
      busy = false;
    }
  }

  async function refresh() {
    if (!device) return;
    try {
      status = await api.status();
    } catch {
      status = null;
    }
  }

  onMount(() => {
    connect();
    const t = setInterval(refresh, 5000);
    return () => clearInterval(t);
  });

  const inkColor: Record<string, string> = { black: "#2b2b2b", cyan: "#1ba1d6", magenta: "#d6338a", yellow: "#e9c21b" };
  const swatch = (name: string) => inkColor[name.toLowerCase().split(/\s/)[0]] ?? "var(--muted)";
  const idle = $derived(status?.scanner === "Idle");
</script>

<section class="device">
  {#if device}
    <button class="who" onclick={() => (editing = !editing)} title="Change device">
      <span class="dot" class:ok={idle} class:off={!status || status.scanner === "Offline"}></span>
      <span class="grow">
        <span class="name">{status?.printer?.model || device.name}</span>
        <span class="sub">{status?.scanner ?? "Checking…"}, {device.host.split(":")[0]}</span>
      </span>
    </button>
    {#if status?.printer}
      {#if status.printer.reasons.length}
        <div class="reasons small">{status.printer.reasons.join(", ")}</div>
      {/if}
      <ul class="ink" title="Ink levels">
        {#each status.printer.ink as i (i.name)}
          <li title="{i.name} {i.level < 0 ? "unknown" : `${i.level}%`}">
            <span class="bar"><span style="height:{Math.max(0, i.level)}%; background:{swatch(i.name)}"></span></span>
            <span class="pct">{i.level < 0 ? "?" : i.level}</span>
          </li>
        {/each}
      </ul>
      <button class="ghost small web" onclick={() => (details = true)}>Device details</button>
    {/if}
    {#if editing}
      <button class="ghost small web" onclick={() => openUrl(`http://${device!.host.split(":")[0]}/`)}>Open device web page</button>
    {/if}
  {:else if busy}
    <p class="muted small searching"><span class="spinner"></span>Looking for a scanner on Wi‑Fi</p>
  {/if}

  {#if error}
    <p class="error small">{error}</p>
  {/if}
  {#if editing || (error && !device)}
    <form class="host" onsubmit={(e) => { e.preventDefault(); connect(host); }}>
      <input type="text" placeholder="IP address, blank to search" bind:value={host} />
      <button disabled={busy}>{busy ? "…" : "Connect"}</button>
    </form>
  {/if}
</section>

{#if details && device}<DeviceDetails host={device.host.split(":")[0]} onclose={() => (details = false)} />{/if}

<style>
  .device { display: grid; gap: 10px; padding: 0 4px; }
  .who { height: auto; justify-content: flex-start; text-align: left; border: none; background: none; padding: 6px; gap: 10px; margin: 0 -2px; }
  .who:hover { background: var(--hover); }
  .grow { flex: 1; min-width: 0; display: grid; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font-size: 12px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--warn); flex: none; box-shadow: 0 0 0 3px color-mix(in srgb, var(--warn) 20%, transparent); }
  .dot.ok { background: var(--ok); box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 20%, transparent); }
  .dot.off { background: var(--danger); box-shadow: 0 0 0 3px color-mix(in srgb, var(--danger) 20%, transparent); }
  .reasons { color: var(--warn); padding: 0 6px; }
  .ink { list-style: none; margin: 0; padding: 0 6px; display: grid; grid-template-columns: repeat(auto-fit, minmax(0, 1fr)); gap: 8px; }
  .ink li { display: grid; gap: 4px; justify-items: center; }
  .bar { width: 100%; height: 34px; background: var(--sunken); border: 1px solid var(--line); border-radius: 4px; overflow: hidden; display: flex; align-items: flex-end; }
  .bar span { display: block; width: 100%; }
  .pct { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .web { justify-self: start; }
  .searching { display: flex; gap: 8px; align-items: center; padding: 6px; }
  .host { display: flex; gap: 6px; }
</style>
