<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fileImg, size, type ShrinkResult } from "./api";

  let { path, onclose, ondone }: { path: string; onclose: () => void; ondone: () => void } = $props();

  let mb = $state(1);
  let busy = $state(false);
  let error = $state("");
  // A shrunk draft waiting for the user to look at it, then save or try again.
  let draft = $state<ShrinkResult | null>(null);
  let saved = $state<string | null>(null);
  let page = $state(0);
  let showing = $state<"Original" | "Shrunk">("Shrunk");
  let zoom = $state(false);

  const presets = [0.5, 1, 2, 5, 10];
  const name = (p: string) => p.split("/").pop();

  async function run() {
    busy = true;
    error = "";
    try {
      draft = await api.shrinkPdf(path, mb);
      page = 0;
      showing = "Shrunk";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function save() {
    if (!draft) return;
    busy = true;
    try {
      saved = await api.keepShrunk(path, draft.out);
      draft = null;
      ondone();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function retry() {
    if (draft) await api.discardShrunk(draft.out).catch(() => {});
    draft = null;
  }

  async function cancel() {
    if (busy) return;
    await retry();
    onclose();
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") cancel();
    else if (draft && e.key === "ArrowRight") page = Math.min(draft.pages - 1, page + 1);
    else if (draft && e.key === "ArrowLeft") page = Math.max(0, page - 1);
    else if (draft && e.key === " ") showing = showing === "Shrunk" ? "Original" : "Shrunk";
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop">
  <div class="card dialog" class:wide={!!draft} role="dialog" aria-label="Shrink PDF">
    <h2>Shrink PDF</h2>
    <p class="muted file">{name(path)}</p>

    {#if saved}
      <p>Saved <button class="ghost link" onclick={() => revealItemInDir(saved!)}>{name(saved)}</button> next to the original.</p>
      <footer><span class="grow"></span><button class="primary" onclick={onclose}>Done</button></footer>
    {:else if draft}
      <div class="bar">
        <div class="seg" title="Flip between them (Space)">
          {#each ["Original", "Shrunk"] as const as v (v)}
            <button class:on={showing === v} onclick={() => (showing = v)}>{v}</button>
          {/each}
        </div>
        <span class="sizes">{size(draft.before)} → <strong>{size(draft.after)}</strong></span>
        <span class="grow"></span>
        {#if draft.pages > 1}
          <div class="pager">
            <button class="ghost small" onclick={() => (page = Math.max(0, page - 1))} disabled={page === 0} aria-label="Previous page">‹</button>
            <span class="muted small">Page {page + 1} of {draft.pages}</span>
            <button class="ghost small" onclick={() => (page = Math.min(draft!.pages - 1, page + 1))} disabled={page === draft.pages - 1} aria-label="Next page">›</button>
          </div>
        {/if}
        <div class="seg zoom">
          <button class:on={!zoom} onclick={() => (zoom = false)}>Fit</button>
          <button class:on={zoom} onclick={() => (zoom = true)}>Zoom</button>
        </div>
      </div>
      <div class="view" class:zoom>
        {#key `${showing}-${page}-${zoom}`}
          <img src={fileImg(showing === "Shrunk" ? draft.out : path, page, zoom ? 2400 : 1200)} alt="{showing}, page {page + 1}" />
        {/key}
      </div>
      {#if !draft.fits}
        <p class="warn small">This PDF can't get under {mb} MB. This is the smallest it goes, {size(draft.after)}.</p>
      {:else}
        <p class="muted small">Flip between Original and Shrunk, and zoom in on small text, before saving.</p>
      {/if}
      {#if error}<p class="error">{error}</p>{/if}
      <footer>
        <button class="ghost" onclick={cancel} disabled={busy}>Cancel</button>
        <span class="grow"></span>
        <button onclick={retry} disabled={busy}>Try another size</button>
        <button class="primary" onclick={save} disabled={busy}>{busy ? "Saving…" : "Save"}</button>
      </footer>
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
      <p class="muted small">Scanned images are reduced in resolution and quality only as much as needed; text stays sharp. You'll see it before anything is saved.</p>
      {#if error}<p class="error">{error}</p>{/if}
      <footer>
        <button class="ghost" onclick={cancel} disabled={busy}>Cancel</button>
        <span class="grow"></span>
        <button class="primary" onclick={run} disabled={busy || !(mb > 0)}>{busy ? "Shrinking…" : "Shrink"}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .dialog { width: min(460px, 92vw); max-height: 92vh; padding: 20px 22px; display: grid; gap: 14px; }
  .dialog.wide { width: min(820px, 94vw); }
  .file { font-size: 12px; word-break: break-all; margin-top: -8px; }
  .row { display: flex; align-items: center; gap: 8px; }
  input[type="number"] { width: 90px; }
  .presets { display: flex; gap: 6px; flex-wrap: wrap; }
  .presets button { font-size: 12px; height: 24px; padding: 0 10px; }
  .presets button.on { border-color: var(--accent); background: var(--accent-soft); }
  .small { font-size: 12px; }
  p { margin: 0; }
  .link { color: var(--accent); padding: 0; height: auto; border: none; background: none; }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
  .error { color: var(--danger); }
  .warn { color: var(--warn); }
  .bar { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  .bar .seg { width: 170px; }
  .bar .seg.zoom { width: 110px; }
  .sizes { font-variant-numeric: tabular-nums; }
  .pager { display: flex; align-items: center; gap: 2px; }
  .pager button { width: 24px; padding: 0; font-size: 16px; }
  /* Fixed height so flipping Original/Shrunk keeps the page in the same place. */
  .view { height: min(58vh, 640px); background: var(--sunken); border-radius: 8px; display: grid; place-items: center; overflow: hidden; padding: 12px; }
  .view img { max-width: 100%; max-height: calc(min(58vh, 640px) - 24px); object-fit: contain; background: var(--paper); box-shadow: 0 0 0 1px var(--line-strong); }
  .view.zoom { place-items: start center; overflow: auto; }
  .view.zoom img { max-width: none; max-height: none; width: 1200px; }
</style>
