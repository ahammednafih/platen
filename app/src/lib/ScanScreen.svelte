<script lang="ts">
  import { api, dur, img, mm, type PageResult, type SessionView } from "./api";
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
    const r = await run(replace === undefined ? "Scanning A4 page" : `Rescanning page ${replace + 1}`,
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
    if (busy || wizard || preview !== null || e.target instanceof HTMLInputElement) return;
    const n = session.pages.length;
    if (!n) return;
    const i = selected ?? 0;
    if (e.key === "ArrowRight") selected = Math.min(n - 1, i + 1);
    else if (e.key === "ArrowLeft") selected = Math.max(0, i - 1);
    else if (e.key === " " && selected !== null) preview = selected;
    else if ((e.key === "Backspace" || e.key === "Delete") && selected !== null) remove(selected);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="screen">
  <header>
    <div class="title">
      <button class="ghost back" onclick={onclose} disabled={!!busy} title="Back to start (keeps the draft)">←</button>
      <div>
        <h1>{session.name}</h1>
        <div class="muted small">
          {session.pages.length} {session.pages.length === 1 ? "page" : "pages"}{totalSecs ? ` · ${dur(totalSecs)} scanning` : ""}
        </div>
      </div>
    </div>
    <div class="actions">
      <button class="primary" onclick={() => scanA4()} disabled={!connected || !!busy}>Scan A4</button>
      <button onclick={() => (wizard = {})} disabled={!connected || !!busy}>Long page…</button>
      <button onclick={onexport} disabled={!!busy || !session.pages.length}>Export…</button>
    </div>
  </header>

  <div class="status" class:err={log && !log.ok}>
    {#if busy}
      <span class="spinner"></span> {busy}… <span class="muted">{dur((now - started) / 1000)}</span>
    {:else if log}
      {log.ok ? "✓" : "✗"} {log.text}
    {:else if !connected}
      <span class="muted">Waiting for the scanner — you can still review, reorder and export pages.</span>
    {:else}
      <span class="muted">New pages go after the selected one. Drag to reorder.</span>
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
            <span class="muted small">{p.kind === "Long" ? "Long · " : ""}{mm(p).join("×")} mm</span>
          </div>
          {#if selected === i}
            <div class="tools">
              <button class="ghost" title="View larger (Space)" onclick={() => (preview = i)}>View</button>
              <button class="ghost" title="Scan this page again" onclick={() => retake(i)} disabled={!connected || !!busy}>Retake</button>
              <button class="ghost" title="Paint torn or cut-away edges" onclick={() => fill(i)} disabled={!!busy}>Fill</button>
              <button class="ghost danger" title="Delete (⌫)" onclick={() => remove(i)} disabled={!!busy}>Delete</button>
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  {:else}
    <div class="empty">
      <p>Put a page face down on the glass and press <strong>Scan A4</strong>.</p>
      <p class="muted">For pages longer than A4, use <strong>Long page</strong> — it scans the top and bottom halves and joins them.</p>
    </div>
  {/if}
</div>

{#if wizard}
  <LongPageWizard
    replace={wizard.replace}
    at={insertAt}
    oncancel={() => (wizard = null)}
    ondone={(r) => { wizard = null; landed(r); }}
  />
{/if}

{#if preview !== null}
  <Lightbox pages={session.pages} bind:index={preview} onclose={() => { selected = preview; preview = null; }} />
{/if}

<style>
  .screen { padding: 20px 28px 40px; display: grid; gap: 14px; align-content: start; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 16px; }
  .title { display: flex; gap: 10px; align-items: center; min-width: 0; }
  .title h1 { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .back { font-size: 18px; padding: 2px 8px; }
  .actions { display: flex; gap: 8px; flex: none; }
  .small { font-size: 12px; }
  .status { min-height: 22px; font-size: 13px; color: var(--ok); display: flex; gap: 6px; align-items: center; }
  .status.err { color: var(--danger); }
  .status:has(.spinner) { color: var(--text); }
  .spinner {
    width: 12px; height: 12px; border-radius: 50%; border: 2px solid var(--line); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 18px; }
  .tile { display: grid; gap: 6px; align-content: start; border-radius: var(--radius); padding: 6px; border: 2px solid transparent; }
  .tile.sel { border-color: var(--accent); background: var(--panel); }
  .tile.over { border-color: var(--warn); border-style: dashed; }
  .thumb {
    padding: 0; border: 1px solid var(--line); border-radius: 4px; background: #fff;
    aspect-ratio: 210 / 297; overflow: hidden; display: grid; place-items: center; box-shadow: var(--shadow);
  }
  .thumb img { max-width: 100%; max-height: 100%; object-fit: contain; display: block; }
  .meta { display: flex; gap: 8px; align-items: baseline; padding: 0 2px; }
  .num { font-weight: 600; }
  .tools { display: flex; flex-wrap: wrap; margin: 0 -4px; }
  .tools button { padding: 2px 6px; font-size: 12px; color: var(--accent); }
  .tools button.danger { color: var(--danger); }
  .empty { margin: 60px auto; max-width: 420px; text-align: center; }
  .empty p { margin: 0 0 8px; }
</style>
