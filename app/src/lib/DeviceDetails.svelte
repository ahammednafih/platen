<script lang="ts">
  import { onMount } from "svelte";
  import { api, type DetailGroup } from "./api";

  let { onclose }: { onclose: () => void } = $props();

  let groups = $state<DetailGroup[]>([]);
  let asking = $state(false);
  let password = $state("");
  let error = $state("");
  let busy = $state(false);

  async function load(pw?: string) {
    busy = true;
    error = "";
    try {
      groups = await api.deviceDetails(pw);
      asking = false;
      password = "";
    } catch (e) {
      const msg = String(e);
      asking = msg === "needs-password" || msg === "wrong-password";
      error = msg === "wrong-password" ? "That password didn't work." : asking ? "" : msg;
    } finally {
      busy = false;
    }
  }

  async function forget() {
    await api.deviceForget().catch(() => {});
    groups = [];
    asking = true;
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

    {#if groups.length}
      <button class="ghost small forget" onclick={forget}>Forget saved password</button>
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
  dd { font-variant-numeric: tabular-nums; text-align: right; }
  .forget { justify-self: start; }
</style>
