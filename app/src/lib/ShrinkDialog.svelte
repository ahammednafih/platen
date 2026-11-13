<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fileImg, size, type ShrinkResult } from "./api";
  import Modal from "./Modal.svelte";
  import Viewer from "./Viewer.svelte";

  let { path, onclose, ondone }: { path: string; onclose: () => void; ondone: () => void } = $props();

  let mb = $state(1);
  let limit = $state(true);
  let busy = $state(false);
  let error = $state("");
  // A shrunk draft waiting for the user to look at it, then save or try again.
  let draft = $state<ShrinkResult | null>(null);
  let saved = $state<string | null>(null);
  let page = $state(0);
  let showing = $state<"Original" | "Shrunk">("Shrunk");
  let zoom = $state(false);
  let count = $state(0);
  let skip = $state<number[]>([]);
  let thumbs = $state<Record<number, boolean>>({});
  let typed = $state("");
  let typedBad = $state(false);
  let viewing = $state<number | null>(null);
  const THUMB_W = 240;
  // Draft page i is this page of the original.
  const kept = $derived(Array.from({ length: count }, (_, i) => i).filter((i) => !skip.includes(i + 1)));

  $effect(() => {
    api.filePages(path).then((n) => {
      count = n;
      if (n > 1) api.pdfPreviews(path, THUMB_W, (r) => (thumbs[r.page] = r.ok)).catch(() => {});
    }, () => {});
  });

  function ranges(pages: number[]) {
    const s = [...pages].sort((a, b) => a - b);
    const out: string[] = [];
    for (let i = 0; i < s.length; i++) {
      const start = s[i];
      while (s[i + 1] === s[i] + 1) i++;
      out.push(start === s[i] ? `${start}` : `${start}-${s[i]}`);
    }
    return out.join(", ");
  }

  function parse(text: string): number[] | null {
    const out = new Set<number>();
    for (const part of text.split(",").map((p) => p.trim()).filter(Boolean)) {
      const m = part.match(/^(\d+)\s*(?:-\s*(\d*))?$/);
      if (!m) return null;
      const a = +m[1];
      const b = m[2] === undefined ? a : m[2] === "" ? count : +m[2];
      if (a < 1 || b < a || b > count) return null;
      for (let p = a; p <= b; p++) out.add(p);
    }
    return [...out];
  }

  function toggle(n: number) {
    skip = skip.includes(n) ? skip.filter((p) => p !== n) : [...skip, n];
    typed = ranges(skip);
    typedBad = false;
  }

  function onType() {
    const p = parse(typed);
    typedBad = p === null;
    if (p) skip = p;
  }

  const presets = [0.5, 1, 2, 5, 10];
  const name = (p: string) => p.split("/").pop();

  async function run() {
    busy = true;
    error = "";
    try {
      draft = await api.shrinkPdf(path, limit ? mb : null, skip);
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
    if (draft && e.key === "ArrowRight") page = Math.min(draft.pages - 1, page + 1);
    else if (draft && e.key === "ArrowLeft") page = Math.max(0, page - 1);
    else if (draft && e.key === " ") showing = showing === "Shrunk" ? "Original" : "Shrunk";
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<Modal label="Shrink PDF" onclose={cancel} dismissible={!busy}>
  <div class="card dialog" class:wide={!!draft}>
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
          <img src={fileImg(showing === "Shrunk" ? draft.out : path, showing === "Shrunk" ? page : (kept[page] ?? page), zoom ? 2400 : 1200)} alt="{showing}, page {page + 1}" />
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
        <input type="number" min="0.05" step="0.1" bind:value={mb} onfocus={() => (limit = true)} class:off={!limit} />
        <span>MB</span>
      </div>
      <div class="presets">
        {#each presets as p (p)}
          <button class:on={limit && mb === p} onclick={() => ((mb = p), (limit = true))}>{p} MB</button>
        {/each}
        <button class:on={!limit} onclick={() => (limit = false)}>Any size</button>
      </div>
      {#if count > 1}
        <div class="row">
          <label for="skip-pages">Leave out</label>
          <input id="skip-pages" type="text" placeholder="e.g. 11-18" bind:value={typed} oninput={onType} class:bad={typedBad} />
          <span class="muted small count">{skip.length ? `Keeping ${kept.length} of ${count}` : `All ${count} pages`}</span>
        </div>
        <div class="pages">
          {#each { length: count } as _, i (i)}
            {@const out = skip.includes(i + 1)}
            <div class="tile">
              <button class="thumb" class:out aria-pressed={!out} aria-label="Page {i + 1}{out ? ', left out' : ''}" onclick={() => toggle(i + 1)}>
                <span class="frame">
                  {#if thumbs[i]}
                    <img src={fileImg(path, i, THUMB_W)} alt="" />
                  {:else if thumbs[i] === undefined}
                    <span class="spinner"></span>
                  {/if}
                  {#if out}<span class="x" aria-hidden="true">✕</span>{/if}
                </span>
                <span class="num">{i + 1}</span>
              </button>
              <button class="expand" aria-label="View page {i + 1}" title="View page" onclick={() => (viewing = i)}>
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5 9 7M2.5 13.5 7 9" /></svg>
              </button>
            </div>
          {/each}
        </div>
      {/if}
      <p class="muted small">{limit ? "Scanned images are reduced in resolution and quality only as much as needed; text stays sharp." : "Pages are left out; the rest stay as they are."} You'll see it before anything is saved.</p>
      {#if error}<p class="error">{error}</p>{/if}
      <footer>
        <button class="ghost" onclick={cancel} disabled={busy}>Cancel</button>
        <span class="grow"></span>
        <button class="primary" onclick={run} disabled={busy || (limit ? !(mb > 0) : !skip.length) || (count > 0 && kept.length === 0)}>{busy ? "Working…" : limit ? "Shrink" : "Leave out pages"}</button>
      </footer>
    {/if}
  </div>
</Modal>

{#if viewing !== null}
  <Viewer {path} bind:page={viewing} onclose={() => (viewing = null)}>
    {#snippet actions(p: number)}
      {@const out = skip.includes(p + 1)}
      <button class="ghost viewer-skip" aria-pressed={out} onclick={() => toggle(p + 1)}>{out ? "Left out ✕" : "Leave out"}</button>
    {/snippet}
  </Viewer>
{/if}

<style>
  .dialog { width: min(520px, 92vw); max-height: 92vh; padding: 20px 22px; display: grid; gap: 14px; }
  .dialog.wide { width: min(820px, 94vw); }
  .file { font-size: 12px; word-break: break-all; margin-top: -8px; }
  .row { display: flex; align-items: center; gap: 8px; }
  input[type="number"] { width: 90px; }
  input.off { opacity: 0.5; }
  .presets { display: flex; gap: 6px; flex-wrap: wrap; }
  .presets button { font-size: 12px; height: 24px; padding: 0 10px; }
  .presets button.on { border-color: var(--accent); background: var(--accent-soft); }
  .small { font-size: 12px; }
  .row label { flex: none; }
  .row input[type="text"] { flex: 1; }
  .row .count { flex: none; font-variant-numeric: tabular-nums; }
  input.bad { border-color: var(--danger); }
  .pages { display: grid; grid-template-columns: repeat(auto-fill, minmax(84px, 1fr)); grid-auto-rows: max-content; gap: 10px 8px; max-height: 300px; overflow: auto; padding: 4px; }
  .thumb { display: flex; flex-direction: column; align-items: stretch; gap: 4px; width: 100%; height: auto; padding: 0; border: none; background: none; }
  .thumb:hover:not(:disabled) { background: none; }
  .frame { position: relative; width: 100%; aspect-ratio: 3 / 4; display: grid; place-items: center; overflow: hidden; border-radius: 4px; background: var(--paper); box-shadow: 0 0 0 1px var(--line-strong); }
  .thumb:hover .frame { box-shadow: 0 0 0 2px var(--accent); }
  .frame img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
  .num { font-size: 11px; color: var(--muted); font-variant-numeric: tabular-nums; }
  .thumb.out img { opacity: 0.2; }
  .thumb.out .num { text-decoration: line-through; }
  .tile { position: relative; }
  .expand { position: absolute; top: 4px; right: 4px; width: 24px; height: 24px; padding: 0; border-radius: 6px; background: var(--raised); opacity: 0; }
  .tile:hover .expand, .expand:focus-visible { opacity: 1; }
  .viewer-skip { color: #eee; }
  .viewer-skip[aria-pressed="true"] { color: #f0a59c; }
  .x { position: absolute; inset: 0; display: grid; place-items: center; font-size: 22px; color: var(--danger); }
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
