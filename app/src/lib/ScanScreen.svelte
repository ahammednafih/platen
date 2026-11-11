<script lang="ts">
  import { api, covered, dur, img, mm, type Edit, type PageResult, type SessionView } from "./api";
  import AdjustDialog from "./AdjustDialog.svelte";
  import LongPageWizard from "./LongPageWizard.svelte";
  import Lightbox from "./Lightbox.svelte";

  let { session = $bindable(), connected, onexport, onclose }: {
    session: SessionView;
    connected: boolean;
    onexport: () => void;
    onclose: () => void;
  } = $props();

  let selected = $state<number | null>(session.pages.length ? session.pages.length - 1 : null);
  let busy = $state("");
  let started = $state(0);
  let now = $state(Date.now());
  let log = $state<{ ok: boolean; text: string } | null>(null);
  let wizard = $state<{ replace?: number } | null>(null);
  let preview = $state<number | null>(null);
  let adjusting = $state<number | null>(null);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  $effect(() => {
    if (!busy) return;
    const t = setInterval(() => (now = Date.now()), 250);
    return () => clearInterval(t);
  });

  const insertAt = $derived(selected === null ? undefined : selected + 1);
  const totalSecs = $derived(session.pages.reduce((a, p) => a + p.secs, 0));

  async function run<T>(label: string, f: () => Promise<T>): Promise<T | undefined> {
    busy = label;
    started = now = Date.now();
    try {
      return await f();
    } catch (e) {
      log = { ok: false, text: String(e) };
    } finally {
      busy = "";
    }
  }

  function landed(r: PageResult) {
    session = r.session;
    selected = r.index;
    log = { ok: true, text: `Page ${r.index + 1}: ${r.note}` };
  }

  async function scanA4(replace?: number) {
    const r = await run(replace === undefined ? "Scanning page" : `Rescanning page ${replace + 1}`,
      () => api.scanA4({ replace, at: insertAt }));
    if (r) landed(r);
  }

  function retake(i: number) {
    if (session.pages[i].kind === "Long") wizard = { replace: i };
    else scanA4(i);
  }

  async function fill(i: number) {
    const r = await run("Filling torn areas", () => api.fillPage(i));
    if (r) landed(r);
  }

  const editLabels: Record<string, string> = {
    RotateLeft: "Rotating", RotateRight: "Rotating", Rotate180: "Rotating",
    Straighten: "Straightening", Crop: "Cropping", Levels: "Adjusting",
  };

  async function edit(i: number, e: Edit) {
    const r = await run(editLabels[typeof e === "string" ? e : "Levels"], () => api.editPage(i, e));
    if (r) landed(r);
  }

  async function remove(i: number) {
    const s = await run("", () => api.deletePage(i));
    if (!s) return;
    session = s;
    selected = s.pages.length ? Math.min(i, s.pages.length - 1) : null;
    log = { ok: true, text: `Deleted page ${i + 1}` };
  }

  async function move(from: number, to: number) {
    if (from === to) return;
    const s = await run("", () => api.movePage(from, to));
    if (!s) return;
    session = s;
    selected = to;
  }

  function onkey(e: KeyboardEvent) {
    if (busy || covered()) return;
    if (e.metaKey) return command(e);
    if (e.target instanceof HTMLInputElement) return;
    const n = session.pages.length;
    if (!n) return;
    const i = selected ?? 0;
    if (e.key === "ArrowRight") selected = Math.min(n - 1, i + 1);
    else if (e.key === "ArrowLeft") selected = Math.max(0, i - 1);
    else if (e.key === " " && selected !== null) preview = selected;
    else if ((e.key === "Backspace" || e.key === "Delete") && selected !== null) remove(selected);
    else if (e.key === "r" && selected !== null) edit(selected, "RotateRight");
    else if (e.key === "R" && selected !== null) edit(selected, "RotateLeft");
    else return;
    e.preventDefault();
  }

  function command(e: KeyboardEvent) {
    if (e.key === "Enter" && connected) scanA4();
    else if (e.key === "l" && connected) wizard = {};
    else if (e.key === "e" && session.pages.length) onexport();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="screen">
  <header class="topbar">
    <button class="ghost crumb" onclick={onclose} disabled={!!busy} title="Back to drafts (keeps this one)">Scan</button>
    <span class="faint">/</span>
    <h1>{session.name}</h1>
    <span class="count faint small">
      {session.pages.length} {session.pages.length === 1 ? "page" : "pages"}{totalSecs ? `, ${dur(totalSecs)} scanning` : ""}
    </span>
    <span class="grow"></span>
    <button onclick={onexport} disabled={!!busy || !session.pages.length}>Export…</button>
    <button onclick={() => (wizard = {})} disabled={!connected || !!busy} title="For pages taller than the glass, like Legal or receipts">Long page…</button>
    <button class="primary" onclick={() => scanA4()} disabled={!connected || !!busy} title="Scan one page at the size set on the right">Scan page</button>
  </header>
  <div class="content">

  <div class="status" class:err={log && !log.ok}>
    {#if busy}
      <span class="spinner"></span> {busy}… <span class="muted">{dur((now - started) / 1000)}</span>
    {:else if log}
      {log.text}
    {:else if !connected}
      <span>Waiting for the scanner. You can still review, reorder and export pages.</span>
    {:else}
      <span>New pages go after the selected one. Drag to reorder.</span>
    {/if}
  </div>

  {#if session.pages.length}
    <ol class="grid">
      {#each session.pages as p, i (p.file)}
        <li
          class="tile"
          class:sel={selected === i}
          class:over={dragOver === i && dragFrom !== i}
          draggable={!busy}
          ondragstart={(e) => { dragFrom = i; e.dataTransfer!.effectAllowed = "move"; }}
          ondragover={(e) => { e.preventDefault(); dragOver = i; }}
          ondragleave={() => { if (dragOver === i) dragOver = null; }}
          ondrop={(e) => { e.preventDefault(); if (dragFrom !== null) move(dragFrom, i); dragFrom = dragOver = null; }}
          ondragend={() => (dragFrom = dragOver = null)}
        >
          <button class="thumb" onclick={() => (selected = i)} ondblclick={() => (preview = i)}>
            <img src={img("pages", p.file)} alt="Page {i + 1}" loading="lazy" draggable="false" />
          </button>
          <div class="meta">
            <span class="num">{i + 1}</span>
            <span class="muted small">{p.kind === "Long" ? "Long, " : ""}{mm(p).join("×")} mm</span>
          </div>
          {#if selected === i}
            <div class="tools">
              <button class="ghost" title="View larger (Space)" onclick={() => (preview = i)}>View</button>
              <button class="ghost" title="Scan this page again" onclick={() => retake(i)} disabled={!connected || !!busy}>Retake</button>
              <button class="ghost" title="Paint torn or cut-away edges" onclick={() => fill(i)} disabled={!!busy}>Fill</button>
              <button class="ghost" title="Rotate left (⇧R)" aria-label="Rotate left" onclick={() => edit(i, "RotateLeft")} disabled={!!busy}>↺</button>
              <button class="ghost" title="Rotate right (R)" aria-label="Rotate right" onclick={() => edit(i, "RotateRight")} disabled={!!busy}>↻</button>
              <button class="ghost" title="Straighten tilted text" onclick={() => edit(i, "Straighten")} disabled={!!busy}>Straighten</button>
              <button class="ghost" title="Trim the dark lid around the paper (straighten first)" onclick={() => edit(i, "Crop")} disabled={!!busy}>Crop</button>
              <button class="ghost" title="Brightness and contrast" onclick={() => (adjusting = i)} disabled={!!busy}>Adjust</button>
              <button class="ghost danger" title="Delete (⌫)" onclick={() => remove(i)} disabled={!!busy}>Delete</button>
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  {:else}
    <div class="empty">
      <p>Put a page face down on the glass and press <strong>Scan page</strong>.</p>
      <p class="muted">Pick the original's size on the right. For pages taller than the glass, use <strong>Long page</strong>: it scans the top and bottom halves and joins them.</p>
    </div>
  {/if}
  </div>
</div>

{#if wizard}
  <LongPageWizard
    replace={wizard.replace}
    at={insertAt}
    oncancel={() => (wizard = null)}
    ondone={(r) => { wizard = null; landed(r); }}
  />
{/if}

{#if adjusting !== null}
  <AdjustDialog
    page={session.pages[adjusting]}
    number={adjusting + 1}
    oncancel={() => (adjusting = null)}
    onapply={(brightness, contrast) => { const i = adjusting!; adjusting = null; edit(i, { Levels: { brightness, contrast } }); }}
  />
{/if}

{#if preview !== null}
  <Lightbox pages={session.pages} bind:index={preview} onclose={() => { selected = preview; preview = null; }} />
{/if}

<style>
  .crumb { padding: 0 6px; margin-left: -6px; }
  .count { margin-left: 4px; }
  .content { padding: 12px 20px 40px; display: grid; gap: 14px; align-content: start; }
  .status { min-height: 22px; font-size: 12px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .status.err { color: var(--danger); }
  .status:has(.spinner) { color: var(--text); }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 14px; }
  .tile { display: grid; gap: 6px; align-content: start; border-radius: 8px; padding: 8px; border: 1px solid transparent; }
  .tile:hover { background: var(--panel); }
  .tile.sel { border-color: var(--accent); background: var(--accent-soft); }
  .tile.over { border-color: var(--warn); border-style: dashed; }
  .thumb {
    height: auto; padding: 0; border: none; border-radius: 2px; background: var(--paper);
    aspect-ratio: 210 / 297; overflow: hidden; display: grid; place-items: center;
    box-shadow: 0 0 0 1px var(--line-strong), 0 2px 6px rgb(0 0 0 / 8%);
  }
  .thumb:hover:not(:disabled) { background: var(--paper); }
  .thumb img { max-width: 100%; max-height: 100%; object-fit: contain; display: block; }
  .meta { display: flex; gap: 8px; align-items: baseline; padding: 0 2px; }
  .num { font-weight: 600; font-variant-numeric: tabular-nums; }
  .tools { display: flex; flex-wrap: wrap; margin: 0 -4px; }
  .tools button { height: 22px; padding: 0 6px; font-size: 12px; }
  .tools button.danger { color: var(--danger); }
  .empty { margin: 80px auto; max-width: 400px; text-align: center; display: grid; gap: 8px; }
</style>
