<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, size, type ShrinkResult } from "./api";

  let { path, onclose, ondone }: { path: string; onclose: () => void; ondone: () => void } = $props();

  let mb = $state(1);
  let busy = $state(false);
  let error = $state("");
  let done = $state<ShrinkResult | null>(null);

  const presets = [0.5, 1, 2, 5, 10];

  async function run() {
    busy = true;
    error = "";
    try {
      done = await api.shrinkPdf(path, mb);
      ondone();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  function onkey(e: KeyboardEvent) {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onclose();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop">
  <div class="card dialog" role="dialog" aria-label="Shrink PDF">
    <h2>Shrink PDF</h2>
    <p class="muted file">{path}</p>
    {#if done}
      <p>{size(done.before)} → <strong>{size(done.after)}</strong></p>
      <button class="ghost link" onclick={() => revealItemInDir(done!.out)}>{done.out.split("/").pop()}</button>
      <footer><span class="grow"></span><button class="primary" onclick={onclose}>Done</button></footer>
    {:else}
      <div class="row">
        <span>Fit under</span>
        <input type="number" min="0.05" step="0.1" bind:value={mb} />
        <span>MB</span>
      </div>
      <div class="presets">
        {#each presets as p (p)}
          <button class:on={mb === p} onclick={() => (mb = p)}>{p} MB</button>
        {/each}
      </div>
      <p class="muted small">Scanned images are reduced in resolution and quality only as much as needed; text stays sharp. A copy is saved next to the original.</p>
      {#if error}<p class="error">{error}</p>{/if}
      <footer>
        <button class="ghost" onclick={onclose} disabled={busy}>Cancel</button>
        <span class="grow"></span>
        <button class="primary" onclick={run} disabled={busy || !(mb > 0)}>{busy ? "Shrinking…" : "Shrink"}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .dialog { width: min(460px, 92vw); padding: 20px 22px; display: grid; gap: 14px; }
  .file { font-size: 12px; word-break: break-all; margin: 0; }
  .row { display: flex; align-items: center; gap: 8px; }
  input[type="number"] { width: 90px; }
  .presets { display: flex; gap: 6px; flex-wrap: wrap; }
  .presets button { font-size: 12px; height: 24px; padding: 0 10px; }
  .presets button.on { border-color: var(--accent); color: var(--accent); background: var(--accent-soft); }
  .small { font-size: 12px; }
  p { margin: 0; }
  .link { color: var(--accent); padding: 0; height: auto; border: none; background: none; justify-self: start; }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
  .error { color: var(--danger); }
</style>
