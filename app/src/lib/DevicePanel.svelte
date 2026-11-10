<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Device, type DeviceStatus } from "./api";

  let { connected = $bindable(false) }: { connected?: boolean } = $props();

  let device = $state<Device | null>(null);
  let status = $state<DeviceStatus | null>(null);
  let error = $state("");
  let busy = $state(false);
  let host = $state("");
  let editing = $state(false);

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
  <h3>Device</h3>
  {#if device}
    <div class="row">
      <span class="dot" class:ok={idle} class:off={!status || status.scanner === "Offline"}></span>
      <div class="grow">
        <div class="name">{status?.printer?.model || device.name}</div>
        <div class="muted small">{device.host} · {status?.scanner ?? "…"}</div>
      </div>
    </div>
    {#if status?.printer}
      {#if status.printer.reasons.length}
        <div class="reasons small">{status.printer.reasons.join(", ")}</div>
      {/if}
      <ul class="ink">
        {#each status.printer.ink as i (i.name)}
          <li>
            <span class="small">{i.name}</span>
            <span class="bar"><span style="width:{Math.max(0, i.level)}%; background:{swatch(i.name)}"></span></span>
            <span class="small muted pct">{i.level < 0 ? "?" : `${i.level}%`}</span>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="actions">
      <button class="ghost small" onclick={() => openUrl(`http://${device!.host.split(":")[0]}/`)}>Web settings</button>
      <button class="ghost small" onclick={() => (editing = !editing)}>Change…</button>
    </div>
  {:else if busy}
    <p class="muted">Looking for the scanner on Wi‑Fi…</p>
  {/if}

  {#if error}
    <p class="error small">{error}</p>
  {/if}
  {#if editing || (error && !device)}
    <form class="host" onsubmit={(e) => { e.preventDefault(); connect(host); }}>
      <input type="text" placeholder="IP address (blank = search)" bind:value={host} />
      <button disabled={busy}>{busy ? "…" : "Connect"}</button>
    </form>
  {/if}
</section>

<style>
  .device { display: grid; gap: 10px; }
  .row { display: flex; gap: 10px; align-items: center; }
  .grow { flex: 1; min-width: 0; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .small { font-size: 12px; }
  .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--warn); flex: none; }
  .dot.ok { background: var(--ok); }
  .dot.off { background: var(--danger); }
  .reasons { color: var(--warn); }
  .ink { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .ink li { display: grid; grid-template-columns: 64px 1fr 34px; align-items: center; gap: 8px; text-transform: capitalize; }
  .bar { height: 6px; background: var(--sunken); border-radius: 3px; overflow: hidden; }
  .bar span { display: block; height: 100%; border-radius: 3px; }
  .pct { text-align: right; }
  .actions { display: flex; gap: 4px; margin-left: -8px; }
  .actions button { padding: 3px 8px; color: var(--accent); }
  .host { display: flex; gap: 6px; }
  .error { color: var(--danger); margin: 0; }
  p { margin: 0; }
</style>
