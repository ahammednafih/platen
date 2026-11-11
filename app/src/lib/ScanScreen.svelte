<script lang="ts">
  import { untrack } from "svelte";
  import { api, covered, dur, img, mm, type Edit, type PageResult, type SessionView } from "./api";
  import AdjustDialog from "./AdjustDialog.svelte";
  import CropDialog from "./CropDialog.svelte";
  import LongPageWizard from "./LongPageWizard.svelte";
  import Lightbox from "./Lightbox.svelte";

  let { session = $bindable(), connected, press = null, onpressed = () => {}, onprint = () => {}, onexport, onclose }: {
    session: SessionView;
    connected: boolean;
    /// A press of the device's Scan button, cleared with `onpressed` once taken.
    press?: number | null;
    onpressed?: () => void;
    /// Hand a PDF of this scan to the Print screen.
    onprint?: (path: string) => void;
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
  let cropping = $state<number | null>(null);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  $effect(() => {
    if (!busy) return;
    const t = setInterval(() => (now = Date.now()), 250);
    return () => clearInterval(t);
  });

  // 16px line icons drawn in currentColor, matching the sidebar's.
  const svg = (d: string) => `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${d}"/></svg>`;
  const icon = {
    view: svg("M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8z M8 6.2a1.8 1.8 0 1 0 0 3.6 1.8 1.8 0 0 0 0-3.6z"),
    retake: svg("M13 8a5 5 0 1 1-1.46-3.54 M13.2 2.6v2.6h-2.6"),
    // A page with an arrow curving over its corner, as in Preview.
    left: svg("M3.5 7.5h6v6h-6z M12.5 10V8a4.5 4.5 0 0 0-4.5-4.5H5.5 M7 2 5.5 3.5 7 5"),
    right: svg("M6.5 7.5h6v6h-6z M3.5 10V8A4.5 4.5 0 0 1 8 3.5h2.5 M9 2l1.5 1.5L9 5"),
    straighten: svg("M2 11 14 5.5 M2 13.5h12"),
    crop: svg("M4.5 1.5v10h10 M1.5 4.5h10v10"),
    fill: svg("M2.8 8.6 7.6 3.8l4 4-4.8 4.8a1 1 0 0 1-1.4 0L2.8 10a1 1 0 0 1 0-1.4z M12.8 10.6s1.4 1.6 1.4 2.4a1.4 1.4 0 0 1-2.8 0c0-.8 1.4-2.4 1.4-2.4z"),
    adjust: svg("M2.5 5h11 M2.5 11h11 M5.5 3.3v3.4 M10.5 9.3v3.4"),
    undo: svg("M5.5 3.5 2.5 6.5l3 3 M2.5 6.5h7a4 4 0 0 1 0 8H7"),
    redo: svg("M10.5 3.5l3 3-3 3 M13.5 6.5h-7a4 4 0 0 0 0 8H9"),
    bw: svg("M8 2.5a5.5 5.5 0 1 0 0 11z M8 2.5a5.5 5.5 0 0 1 0 11"),
    trash: svg("M3 4.5h10 M6.5 4.5V3h3v1.5 M4.4 4.5l.6 9h6l.6-9"),
  };

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

  // A second press while scanning is dropped; the device gives up on it by itself.
  $effect(() => {
    if (press === null) return;
    onpressed();
    untrack(() => {
      if (busy) return;
      run("Scanning from the device button", () => api.scanButton(insertAt)).then((r) => r && landed(r));
    });
  });

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
    Straighten: "Straightening", Crop: "Cropping", CropTo: "Cropping", Levels: "Adjusting",
    Gray: "Converting", BlackWhite: "Converting", Colour: "Converting",
  };

  async function edit(i: number, e: Edit) {
    const r = await run(editLabels[typeof e === "string" ? e : Object.keys(e)[0]], () => api.editPage(i, e));
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

  // The name is the scan's folder name; renaming also renames exports that carry it.
  let renaming = $state(false);
  let draft = $state("");

  function startRename() {
    draft = session.name;
    renaming = true;
  }

  async function commitRename() {
    if (!renaming) return;
    renaming = false;
    const name = draft.trim();
    if (!name || name === session.name) return;
    try {
      session = await api.renameSession(name);
      log = { ok: true, text: `Renamed to “${name}”.` };
    } catch (e) {
      log = { ok: false, text: String(e) };
    }
  }

  function renameKey(e: KeyboardEvent) {
    if (e.key === "Enter") commitRename();
    else if (e.key === "Escape") renaming = false;
    else return;
    e.preventDefault();
    e.stopPropagation();
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

  // Page changes only; in a text field ⌘Z stays the field's own undo.
  async function step(forward: boolean) {
    if (busy || !(forward ? session.can_redo : session.can_undo)) return;
    try {
      session = forward ? await api.redo() : await api.undo();
      if (selected !== null) selected = session.pages.length ? Math.min(selected, session.pages.length - 1) : null;
      log = { ok: true, text: forward ? "Redone." : "Undone." };
    } catch (e) {
      log = { ok: false, text: String(e) };
    }
  }

  async function print() {
    const path = await run("Preparing to print", () => api.scanToPrint());
    if (path) onprint(path);
  }

  function command(e: KeyboardEvent) {
    if (e.key.toLowerCase() === "z" && !(e.target instanceof HTMLInputElement)) step(e.shiftKey);
    else if (e.key === "Enter" && connected) scanA4();
    else if (e.key === "l" && connected) wizard = {};
    else if (e.key === "e" && session.pages.length) onexport();
    else if (e.key === "p" && session.pages.length) print();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="screen">
  <header class="topbar">
    <button class="ghost crumb" onclick={onclose} disabled={!!busy} title="Back to drafts (keeps this one)">Scan</button>
    <span class="faint">/</span>
    {#if renaming}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="rename" aria-label="Scan name" bind:value={draft} autofocus onfocus={(e) => e.currentTarget.select()}
        onkeydown={renameKey} onblur={commitRename} size={Math.max(12, draft.length + 2)} />
    {:else}
      <h1>
        <button class="ghost title" onclick={startRename} disabled={!!busy} title="Rename this scan">
          {session.name}
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M10.5 2.5l3 3-8 8H2.5v-3z M9 4l3 3" /></svg>
        </button>
      </h1>
    {/if}
    <span class="count faint small">
      {session.pages.length} {session.pages.length === 1 ? "page" : "pages"}{totalSecs ? `, ${dur(totalSecs)} scanning` : ""}
    </span>
    <span class="grow"></span>
    <div class="history">
      <button class="ghost square" title="Undo (⌘Z)" aria-label="Undo" onclick={() => step(false)} disabled={!!busy || !session.can_undo}>{@html icon.undo}</button>
      <button class="ghost square" title="Redo (⇧⌘Z)" aria-label="Redo" onclick={() => step(true)} disabled={!!busy || !session.can_redo}>{@html icon.redo}</button>
    </div>
    <button onclick={print} disabled={!!busy || !session.pages.length} title="Open these pages in Print (⌘P)">Print</button>
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

  {#if session.pages.length && selected !== null && session.pages[selected]}
    {@const i = selected}
    {@const p = session.pages[i]}
    <div class="pagebar" role="toolbar" aria-label="Page {i + 1} tools">
      <div class="which">
        <strong>Page {i + 1}</strong>
        <span class="muted">{p.kind === "Long" ? "Long, " : ""}{mm(p).join(" × ")} mm</span>
      </div>
      <div class="group">
        <button class="ghost small" title="View larger (Space)" onclick={() => (preview = i)}>{@html icon.view} View</button>
        <button class="ghost small" title="Scan this page again" onclick={() => retake(i)} disabled={!connected || !!busy}>{@html icon.retake} Retake</button>
      </div>
      <div class="group">
        <button class="ghost small square" title="Rotate left (⇧R)" aria-label="Rotate left" onclick={() => edit(i, "RotateLeft")} disabled={!!busy}>{@html icon.left}</button>
        <button class="ghost small square" title="Rotate right (R)" aria-label="Rotate right" onclick={() => edit(i, "RotateRight")} disabled={!!busy}>{@html icon.right}</button>
        <button class="ghost small" title="Straighten tilted text" onclick={() => edit(i, "Straighten")} disabled={!!busy}>{@html icon.straighten} Straighten</button>
        <button class="ghost small" title="Crop to the paper, a standard size, or a box you draw" onclick={() => (cropping = i)} disabled={!!busy}>{@html icon.crop} Crop</button>
      </div>
      <div class="group">
        <button class="ghost small" title="Paint torn or cut-away edges" onclick={() => fill(i)} disabled={!!busy}>{@html icon.fill} Fill</button>
        <button class="ghost small" title="Brightness and contrast" onclick={() => (adjusting = i)} disabled={!!busy}>{@html icon.adjust} Adjust</button>
        <button class="ghost small" title="Make this page black and white" onclick={() => edit(i, "BlackWhite")} disabled={!!busy}>{@html icon.bw} B&W</button>
        <button class="ghost small" title="Make this page gray" onclick={() => edit(i, "Gray")} disabled={!!busy}>Gray</button>
        {#if p.color}
          <button class="ghost small" title="Back to the colour scan this page was made from" onclick={() => edit(i, "Colour")} disabled={!!busy}>Colour</button>
        {/if}
      </div>
      <span class="grow"></span>
      <button class="ghost small danger" title="Delete (⌫)" onclick={() => remove(i)} disabled={!!busy}>{@html icon.trash} Delete</button>
    </div>
  {/if}

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
            {#if p.kind === "Long"}<span class="faint small">Long</span>{/if}
          </div>
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

{#if cropping !== null && session.pages[cropping]}
  <CropDialog
    page={session.pages[cropping]}
    index={cropping}
    onclose={() => (cropping = null)}
    onapply={(r) => { const i = cropping!; cropping = null; edit(i, { CropTo: r }); }}
  />
{/if}

{#if preview !== null}
  <Lightbox pages={session.pages} bind:index={preview} onclose={() => { selected = preview; preview = null; }} />
{/if}

<style>
  .crumb { padding: 0 6px; margin-left: -6px; }
  .count { margin-left: 4px; }
  .history { display: flex; gap: 1px; margin-right: 4px; }
  .history .square { width: 28px; padding: 0; }
  .history .square :global(svg) { width: 16px; height: 16px; }
  h1 { display: flex; }
  .title { font: inherit; color: var(--text); height: 28px; padding: 0 6px; margin: 0 -2px; gap: 6px; }
  .title svg { opacity: 0; color: var(--muted); transition: opacity 0.14s var(--ease); }
  .title:hover svg, .title:focus-visible svg { opacity: 1; }
  .rename { font: inherit; font-weight: 600; height: 28px; padding: 0 6px; max-width: 360px; }
  .content { padding: 12px 20px 40px; display: grid; gap: 14px; align-content: start; }
  .status { min-height: 22px; font-size: 12px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .status.err { color: var(--danger); }
  .status:has(.spinner) { color: var(--text); }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 14px; }
  .tile { display: grid; gap: 8px; align-content: start; padding: 4px; }
  /* The sheet itself carries selection: an accent ring around the paper, not a tinted box. */
  .thumb {
    height: auto; padding: 0; border: none; border-radius: 2px; background: var(--paper);
    aspect-ratio: 210 / 297; overflow: hidden; display: grid; place-items: center;
    box-shadow: 0 0 0 1px var(--line-strong), 0 2px 6px rgb(0 0 0 / 8%);
    outline: 2px solid transparent; outline-offset: 3px;
    transition: outline-color 0.15s var(--ease), box-shadow 0.15s var(--ease), transform 0.1s var(--ease);
  }
  .thumb:hover:not(:disabled) { background: var(--paper); box-shadow: 0 0 0 1px var(--line-strong), 0 4px 14px rgb(0 0 0 / 12%); }
  .sel .thumb { outline-color: var(--accent); }
  .over .thumb { outline: 2px dashed var(--warn); }
  .thumb:focus-visible { outline-color: var(--accent); }
  .thumb img { max-width: 100%; max-height: 100%; object-fit: contain; display: block; }
  .meta { display: flex; gap: 6px; align-items: baseline; justify-content: center; }
  .num { font-size: 12px; font-weight: 600; font-variant-numeric: tabular-nums; color: var(--muted); min-width: 20px; text-align: center; padding: 1px 6px; border-radius: 10px; }
  .sel .num { color: var(--accent-text); background: var(--accent); }
  .pagebar {
    position: sticky; top: 0; z-index: 2; display: flex; flex-wrap: wrap; align-items: center; gap: 4px 10px;
    padding: 6px 8px 6px 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--raised); box-shadow: var(--shadow);
  }
  .which { display: flex; gap: 8px; align-items: baseline; margin-right: 4px; font-size: 13px; white-space: nowrap; }
  .group { display: flex; align-items: center; gap: 1px; padding-left: 10px; border-left: 1px solid var(--line); }
  .pagebar button { gap: 5px; }
  .pagebar .square { width: 26px; padding: 0; }
  .pagebar .square :global(svg) { width: 16px; height: 16px; }
  .pagebar .danger { color: var(--danger); }
  .pagebar .grow { flex: 1; }
  .empty { margin: 80px auto; max-width: 400px; text-align: center; display: grid; gap: 8px; }
</style>
