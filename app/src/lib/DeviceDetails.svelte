<script lang="ts">
  import { onMount } from "svelte";
  import { api, type ButtonStatus, type DetailGroup } from "./api";

  let { host, onclose }: { host: string; onclose: () => void } = $props();

  let groups = $state<DetailGroup[]>([]);
  let asking = $state(false);
  let password = $state("");
  let error = $state("");
  let busy = $state(false);
  let button = $state<ButtonStatus | null>(null);
  let switching = $state("");
  let switchError = $state("");

  async function load(pw?: string) {
    busy = true;
    error = "";
    try {
      groups = await api.deviceDetails(pw);
      asking = false;
      password = "";
      button = await api.buttonStatus().catch(() => null);
    } catch (e) {
      const msg = String(e);
      asking = msg === "needs-password" || msg === "wrong-password";
      error = msg === "wrong-password" ? "That password didn't work." : asking ? "" : msg;
    } finally {
      busy = false;
    }
  }

  async function act(label: string, f: () => Promise<unknown>) {
    switching = label;
    switchError = "";
    try {
      await f();
      button = await api.buttonStatus();
    } catch (e) {
      switchError = String(e) === "needs-snmp" ? "Network settings access is still off on the device." : String(e);
    } finally {
      switching = "";
    }
  }

  // The device restarts after the change, so wait for it to come back before reading it again.
  const allowWrite = () => act("Turning on…", async () => {
    await api.enableSnmpWrite();
    for (let i = 0; i < 24; i++) {
      await new Promise((r) => setTimeout(r, 5000));
      if (await api.buttonStatus().catch(() => null)) return;
    }
    throw new Error("The device didn't come back. Try the steps above.");
  });

  const toggle = () => act(button?.on ? "Turning off…" : "Turning on…", () => (button?.on ? api.buttonDisable() : api.buttonEnable()));
  const pointHere = () => act("Updating…", () => api.buttonEnable());

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
      <span class="grow"></span>
      {#if groups.length}
        <button class="ghost small" onclick={() => load()} disabled={busy}>{busy ? "Refreshing…" : "Refresh"}</button>
      {/if}
      <button class="ghost small close" onclick={onclose} title="Close (Esc)" aria-label="Close">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </button>
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

    {#if button}
      <section class="snmp">
        <div class="group-title">Scan button</div>
        {#if button.snmp !== "ReadWrite"}
          <p class="small">
            To use the device's own Scan button, the device has to let this Mac register on the network.
            It's off by default ({button.snmp === "Off" ? "SNMP is disabled" : "SNMP is read-only"}).
          </p>
          <ol class="small steps">
            <li>Open <code>https://{host}</code> in a browser and log in with the device password.</li>
            <li>Go to <strong>Network → Network → Protocol</strong>, then <strong>SNMP</strong>.</li>
            <li>Choose <strong>SNMP v1/v2c read-write access</strong>, press <strong>Submit</strong>, then <strong>Yes</strong> to restart.</li>
          </ol>
          <p class="faint small">
            Any device on your network can then change printer settings over SNMP. The device restarts to apply it,
            so it's away for about a minute.
          </p>
          <div><button onclick={allowWrite} disabled={!!switching}>{switching || "Turn it on for me"}</button></div>
        {:else}
          <label class="check small">
            <input type="checkbox" checked={button.on} onchange={toggle} disabled={!!switching} />
            Scan into this app with the device's Scan button
          </label>
          {#if switching}<p class="muted small"><span class="spinner"></span> {switching}</p>{/if}
          {#if button.on && button.destination !== button.name}
            <p class="small">
              The device sends button scans to <strong>{button.destination ?? "USB"}</strong>, not this Mac (“{button.name}”).
            </p>
            <div><button onclick={pointHere} disabled={!!switching}>Send them to this Mac</button></div>
          {:else if button.on}
            <p class="faint small">
              Press Scan on the device (on panels without a screen, both Copy buttons together). The page lands in the
              open scan, or a new one. This Mac shows up on the device as “{button.name}” while the app is open.
            </p>
          {/if}
        {/if}
        {#if switchError}<p class="error small">{switchError}</p>{/if}
      </section>
    {/if}

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

  </div>
</div>

<style>
  .dialog { width: min(460px, 92vw); max-height: 88vh; overflow-y: auto; padding: 20px 22px; display: grid; gap: 12px; }
  header {
    display: flex; align-items: center; gap: 8px; position: sticky; top: -20px; z-index: 1;
    margin: -20px -22px 0; padding: 16px 22px 10px; background: var(--raised); border-bottom: 1px solid var(--line);
  }
  h2 { margin: 0; }
  .grow { flex: 1; }
  .close { width: 26px; height: 26px; padding: 0; margin-right: -8px; border-radius: 50%; }
  .row { display: flex; gap: 8px; }
  .row input { flex: 1; }
  dl { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 4px 12px; margin: 6px 0 0; }
  dt, dd { margin: 0; }
  dt { color: var(--muted); }
  dt.sub { padding-left: 14px; }
  .snmp { display: grid; gap: 8px; }
  .snmp p { margin: 0; }
  .check { display: flex; gap: 8px; align-items: center; }
  .steps { margin: 0; padding-left: 18px; display: grid; gap: 4px; }
  dd { font-variant-numeric: tabular-nums; text-align: right; }
</style>
