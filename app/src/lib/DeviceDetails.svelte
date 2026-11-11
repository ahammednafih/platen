<script lang="ts">
  import { onMount } from "svelte";
  import { api, type DetailGroup, type SnmpAccess } from "./api";

  let { host, onclose }: { host: string; onclose: () => void } = $props();

  let groups = $state<DetailGroup[]>([]);
  let asking = $state(false);
  let password = $state("");
  let error = $state("");
  let busy = $state(false);
  let snmp = $state<SnmpAccess | null>(null);
  let switching = $state(false);
  let switchError = $state("");

  async function load(pw?: string) {
    busy = true;
    error = "";
    try {
      groups = await api.deviceDetails(pw);
      asking = false;
      password = "";
      snmp = await api.snmpAccess().catch(() => null);
    } catch (e) {
      const msg = String(e);
      asking = msg === "needs-password" || msg === "wrong-password";
      error = msg === "wrong-password" ? "That password didn't work." : asking ? "" : msg;
    } finally {
      busy = false;
    }
  }

  // The device restarts after the change, so wait before reading it back.
  async function allowWrite() {
    switching = true;
    switchError = "";
    try {
      await api.enableSnmpWrite();
      for (let i = 0; i < 24; i++) {
        await new Promise((r) => setTimeout(r, 5000));
        const a = await api.snmpAccess().catch(() => null);
        if (a) { snmp = a; break; }
      }
      if (snmp?.mode !== "ReadWrite") switchError = "The device didn't take the change. Try the steps above.";
    } catch (e) {
      switchError = String(e);
    } finally {
      switching = false;
    }
  }

  function onkey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    onclose();
  }

  onMount(() => load());
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop" role="presentation" onclick={onclose}>
  <div class="card dialog" role="dialog" aria-label="Device details" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
    <header>
      <h2>Device details</h2>
      {#if groups.length}
        <button class="ghost small" onclick={() => load()} disabled={busy}>{busy ? "Refreshing…" : "Refresh"}</button>
      {/if}
    </header>

    {#if asking}
      <form onsubmit={(e) => { e.preventDefault(); load(password); }}>
        <p class="muted small">The device asks for its admin password. It's usually on a label on the device, marked “Pwd”. Saved to your Keychain once it works.</p>
        <div class="row">
          <!-- svelte-ignore a11y_autofocus -->
          <input type="password" placeholder="Device password" bind:value={password} autofocus />
          <button class="primary" disabled={busy || !password}>{busy ? "…" : "Log in"}</button>
        </div>
      </form>
    {:else if busy && !groups.length}
      <p class="muted small"><span class="spinner"></span> Reading the device…</p>
    {/if}
    {#if error}<p class="error small">{error}</p>{/if}

    {#each groups as g, i (i)}
      <section>
        {#if g.title && g.title !== groups[i - 1]?.title}<div class="group-title">{g.title}</div>{/if}
        <dl>
          {#each g.rows as r, j (j)}
            <dt class:sub={r.sub}>{r.label}</dt>
            <dd>{r.value}</dd>
          {/each}
        </dl>
      </section>
    {/each}

    {#if snmp}
      <section class="snmp">
        <div class="group-title">Scan button</div>
        {#if snmp.mode === "ReadWrite"}
          <p class="small">Network settings access is on, so the device's Scan button can send scans to this Mac.</p>
        {:else}
          <p class="small">
            To use the device's own Scan button, the device has to let this Mac register on the network.
            It's off by default ({snmp.mode === "Off" ? "SNMP is disabled" : "SNMP is read-only"}).
          </p>
          <ol class="small steps">
            <li>Open <code>https://{host}</code> in a browser and log in with the device password.</li>
            <li>Go to <strong>Network → Network → Protocol</strong>, then <strong>SNMP</strong>.</li>
            <li>Choose <strong>SNMP v1/v2c read-write access</strong> and press <strong>Submit</strong>.</li>
          </ol>
          <p class="faint small">
            Any device on your network can then change printer settings over SNMP. The device restarts to apply it,
            so it's away for about a minute.
          </p>
          <div><button onclick={allowWrite} disabled={switching}>{switching ? "Turning on…" : "Turn it on for me"}</button></div>
          {#if switchError}<p class="error small">{switchError}</p>{/if}
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .dialog { width: min(460px, 92vw); max-height: 88vh; overflow-y: auto; padding: 20px 22px; display: grid; gap: 12px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; }
  dl { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 4px 12px; margin: 6px 0 0; }
  dt, dd { margin: 0; }
  dt { color: var(--muted); }
  dt.sub { padding-left: 14px; }
  .snmp { display: grid; gap: 8px; }
  .snmp p { margin: 0; }
  .steps { margin: 0; padding-left: 18px; display: grid; gap: 4px; }
  dd { font-variant-numeric: tabular-nums; text-align: right; }
</style>
