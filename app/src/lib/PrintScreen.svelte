<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, covered, paperName, type Plan, type PrintCaps, type PrintJob, type Quality, type Scale, type Subset } from "./api";

  let { connected }: { connected: boolean } = $props();

  const PREVIEWS = 60;
  const printable = ["pdf", "jpg", "jpeg", "png"];

  let files = $state<string[]>([]);
  let caps = $state<PrintCaps | null>(null);
  let ranges = $state("");
  let reverse = $state(false);
  let nup = $state(1);
  let scaleMode = $state<"Fit" | "Actual" | "Percent">("Fit");
  let pct = $state(100);
  let copies = $state(1);
  let collate = $state(true);
  let color = $state(true);
  let media = $state("iso_a4_210x297mm");
  let quality = $state<Quality>("Normal");
  let twoSided = $state(false);

  let plan = $state<Plan | null>(null);
  // Picked pages across all files, in order; the Range field mirrors it.
  let sel = $state<boolean[]>([]);
  let anchor = 0;
  let planError = $state("");
  let previews = $state<string[]>([]);
  // Pages: pick what to print. Sheets: what comes out of the printer (N-up, odd/even, order).
  let view = $state<"Pages" | "Sheets">("Pages");
  let sheetPreviews = $state<string[]>([]);
  let busy = $state("");
  let flip = $state(false);
  let log = $state<{ ok: boolean; text: string } | null>(null);

  const scale = $derived<Scale>(scaleMode === "Percent" ? { Percent: pct } : scaleMode);
  const papers = $derived((caps?.media ?? ["iso_a4_210x297mm", "na_letter_8.5x11in"]).filter((m) => !m.startsWith("custom_")));
  const qualities = $derived<Quality[]>(caps?.qualities.length ? caps.qualities : ["Draft", "Normal", "High"]);

  function job(over: Partial<PrintJob> = {}): PrintJob {
    return {
      files: $state.snapshot(files), ranges, subset: "All", reverse, nup, scale, collate,
      opts: { copies, color, media, scaling: "fit", quality },
      ...over,
    };
  }

  $effect(() => {
    if (!connected) return;
    api.printCaps().then((c) => {
      caps = c;
      if (!c.media.includes(media)) media = c.media_default;
      if (!c.color) color = false;
    }).catch(() => {});
  });

  const none = $derived(sel.length > 0 && !sel.some(Boolean));

  // Re-plan shortly after any setting changes.
  $effect(() => {
    const j = job();
    if (!j.files.length || none) {
      plan = null;
      return;
    }
    let stale = false;
    const t = setTimeout(async () => {
      try {
        const p = await api.printPlan(j);
        if (stale) return;
        plan = p;
        planError = "";
        if (view !== "Sheets") return;
        const urls = await Promise.all(p.order.slice(0, PREVIEWS).map((_, i) => api.printPreview(j, i).catch(() => "")));
        if (stale) return urls.forEach((u) => u && URL.revokeObjectURL(u));
        sheetPreviews.forEach((u) => u && URL.revokeObjectURL(u));
        sheetPreviews = urls;
      } catch (e) {
        if (!stale) { plan = null; planError = String(e); }
      }
    }, 250);
    return () => { stale = true; clearTimeout(t); };
  });

  // The preview shows every page, one per tile, so pages can be picked by clicking.
  $effect(() => {
    // Built by hand so picking pages (ranges) doesn't re-render every preview.
    const j: PrintJob = {
      files: $state.snapshot(files), ranges: "", subset: "All", reverse: false, nup: 1, scale, collate: true,
      opts: { copies: 1, color, media, scaling: "fit", quality },
    };
    if (!j.files.length) {
      sel = [];
      previews = [];
      return;
    }
    let stale = false;
    const t = setTimeout(async () => {
      try {
        const p = await api.printPlan(j);
        if (stale) return;
        if (p.pages !== sel.length) setSel(Array(p.pages).fill(true));
        const urls = await Promise.all(p.order.slice(0, PREVIEWS).map((_, i) => api.printPreview(j, i).catch(() => "")));
        if (stale) return urls.forEach((u) => u && URL.revokeObjectURL(u));
        previews.forEach((u) => u && URL.revokeObjectURL(u));
        previews = urls;
      } catch (e) {
        if (!stale) planError = String(e);
      }
    }, 250);
    return () => { stale = true; clearTimeout(t); };
  });

  // "1-3, 7" for the picked pages; empty when every page is picked.
  function toRanges(s: boolean[]) {
    if (s.every(Boolean)) return "";
    const out: string[] = [];
    for (let i = 0; i < s.length; i++) {
      if (!s[i]) continue;
      let j = i;
      while (s[j + 1]) j++;
      out.push(i === j ? `${i + 1}` : `${i + 1}-${j + 1}`);
      i = j;
    }
    return out.join(", ");
  }

  // Same rules as the printer side: blank = all, open ends allowed. Null while half-typed.
  function fromRanges(text: string, n: number): boolean[] | null {
    const parts = text.split(",").map((p) => p.trim()).filter(Boolean);
    if (!parts.length) return Array(n).fill(true);
    const s = Array(n).fill(false);
    for (const part of parts) {
      const [a, b] = part.includes("-") ? part.split("-").map((x) => x.trim()) : [part, part];
      const lo = a ? Number(a) : 1;
      const hi = b ? Number(b) : n;
      if (!Number.isInteger(lo) || !Number.isInteger(hi) || lo < 1 || lo > hi || lo > n) return null;
      for (let i = lo - 1; i < Math.min(hi, n); i++) s[i] = true;
    }
    return s;
  }

  function setSel(s: boolean[]) {
    sel = s;
    ranges = toRanges(s);
  }

  function pickPreset(v: Subset) {
    setSel(sel.map((_, i) => v === "All" || (v === "Odd") === (i % 2 === 0)));
  }

  const preset = $derived.by<Subset | null>(() => {
    if (sel.every(Boolean)) return "All";
    for (const v of ["Odd", "Even"] as const) if (sel.every((on, i) => on === ((v === "Odd") === (i % 2 === 0)))) return v;
    return null;
  });

  // Click toggles a page; shift-click sets the run from the last clicked page to this one.
  function toggle(i: number, e: MouseEvent) {
    const s = [...sel];
    if (e.shiftKey) {
      const on = !s[i];
      for (let k = Math.min(anchor, i); k <= Math.max(anchor, i); k++) s[k] = on;
    } else s[i] = !s[i];
    anchor = i;
    setSel(s);
  }

  function add(paths: string[]) {
    const ok = paths.filter((p) => printable.includes(p.split(".").pop()?.toLowerCase() ?? ""));
    files = [...files, ...ok.filter((p) => !files.includes(p))];
    if (ok.length < paths.length) log = { ok: false, text: "Only PDFs, JPEGs and PNGs can be printed." };
  }

  async function pick() {
    const r = await open({ multiple: true, filters: [{ name: "PDF and images", extensions: printable }] });
    if (r) add(Array.isArray(r) ? r : [r]);
  }

  // Window drag-drop is off (it breaks page reordering), so files arrive as bytes and get staged.
  let dragging = $state(false);
  const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");

  function dragover(e: DragEvent) {
    if (!hasFiles(e) || busy) return;
    e.preventDefault();
    dragging = true;
  }

  function dragleave(e: DragEvent) {
    if (!(e.currentTarget as Node).contains(e.relatedTarget as Node | null)) dragging = false;
  }

  async function drop(e: DragEvent) {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dragging = false;
    const all = [...(e.dataTransfer?.files ?? [])];
    const ok = all.filter((f) => printable.includes(f.name.split(".").pop()?.toLowerCase() ?? ""));
    if (!ok.length) return void (log = { ok: false, text: "Only PDFs, JPEGs and PNGs can be printed." });
    try {
      add(await Promise.all(ok.map(api.stageFile)));
      if (ok.length < all.length) log = { ok: false, text: "Only PDFs, JPEGs and PNGs can be printed." };
    } catch (err) {
      log = { ok: false, text: String(err) };
    }
  }

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= files.length) return;
    const f = [...files];
    [f[i], f[j]] = [f[j], f[i]];
    files = f;
  }

  let stopping = $state(false);
  async function stop() {
    stopping = true;
    busy = "Stopping";
    await api.stopJob().catch(() => {});
  }
  const failed = (e: unknown) => (String(e) === "stopped" ? { ok: true, text: "Stopped. Nothing else will print." } : { ok: false, text: String(e) });

  async function send(j: PrintJob, label: string) {
    busy = label;
    log = null;
    try {
      const n = await api.print(j);
      log = { ok: true, text: `Sent ${n} ${n === 1 ? "job" : "jobs"} to the printer.` };
      return true;
    } catch (e) {
      log = failed(e);
      return false;
    } finally {
      busy = "";
      stopping = false;
    }
  }

  async function print() {
    if (!twoSided) return send(job(), "Printing");
    if (await send(job({ subset: "Odd" }), "Printing fronts")) flip = true;
  }

  async function backs() {
    flip = false;
    await send(job({ subset: "Even", reverse: !reverse }), "Printing backs");
  }

  function onkey(e: KeyboardEvent) {
    if (!e.metaKey || busy || covered()) return;
    if (e.key === "o") pick();
    else if ((e.key === "p" || e.key === "Enter") && connected && plan && !none && !flip) print();
    else return;
    e.preventDefault();
  }

  onMount(() => {
    const un = listen<{ done: number; total: number }>("print-progress", (e) => {
      if (busy && !stopping) busy = busy.replace(/ \d+\/\d+$/, "") + ` ${e.payload.done}/${e.payload.total}`;
    });
    return () => {
      un.then((f) => f());
      previews.forEach((u) => u && URL.revokeObjectURL(u));
      sheetPreviews.forEach((u) => u && URL.revokeObjectURL(u));
    };
  });

  const name = (p: string) => p.split("/").pop();
  const sheetsOut = $derived(plan ? (twoSided ? plan.sheets + (plan.sheets % 2) : plan.order.length) * Math.max(1, copies) : 0);
  const paperRatio = $derived.by(() => {
    const m = media.match(/_([\d.]+)x([\d.]+)(mm|in)$/);
    return m ? `${m[1]} / ${m[2]}` : "210 / 297";
  });
</script>

<svelte:window onkeydown={onkey} />

<div class="screen" class:dragging role="region" aria-label="Print" ondragover={dragover} ondragleave={dragleave} ondrop={drop}>
  <header class="topbar">
    <h1>Print</h1>
    {#if none}
      <span class="faint small count">No pages picked</span>
    {:else if plan}
      <span class="faint small count">
        {plan.picked} of {plan.pages} {plan.pages === 1 ? "page" : "pages"} on {sheetsOut} {sheetsOut === 1 ? "sheet" : "sheets"}
      </span>
    {/if}
    <span class="grow"></span>
    <button onclick={pick} disabled={!!busy}>Add files…</button>
    <button class="primary" onclick={print} disabled={!connected || !!busy || !plan || none || flip}>
      {twoSided ? "Print fronts" : "Print"}
    </button>
  </header>

  <div class="screen-body">
    <div class="screen-main">
      {#if files.length}
        <ul class="files">
          {#each files as f, i (f)}
            <li>
              <span class="fname" title={f}>{name(f)}</span>
              <button class="ghost small" title="Move up" disabled={i === 0} onclick={() => move(i, -1)}>↑</button>
              <button class="ghost small" title="Move down" disabled={i === files.length - 1} onclick={() => move(i, 1)}>↓</button>
              <button class="ghost small" onclick={() => (files = files.filter((x) => x !== f))}>Remove</button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if busy || log || planError || flip}
        <div class="status" class:err={(log && !log.ok) || (planError && !busy)}>
          {#if busy}
            <span class="spinner"></span>{busy}…
            <button class="ghost small stop" onclick={stop} disabled={stopping}>Stop</button>
          {:else if flip}
            <div class="flip">
              <p><strong>Fronts are printed.</strong> Put the printed stack back in the paper tray, blank side up as the printer feeds it, top edge first.</p>
              <div class="flip-actions">
                <button class="primary" onclick={backs}>Print backs</button>
                <button class="ghost" onclick={() => (flip = false)}>Cancel</button>
              </div>
            </div>
          {:else if planError}
            {planError}
          {:else if log}
            {log.text}
          {/if}
        </div>
      {/if}

      {#if !files.length}
        <button class="drop" onclick={pick}>
          <strong>Choose or drop PDFs and images to print</strong>
          <span class="muted">Pick pages, put several on a sheet, or print both sides by hand</span>
        </button>
      {/if}

      {#if files.length && sel.length}
        <div class="seg view">
          {#each ["Pages", "Sheets"] as const as v (v)}
            <button class:on={view === v} onclick={() => (view = v)}>{v}</button>
          {/each}
        </div>
      {/if}

      {#if files.length && view === "Sheets"}
        {#if plan}
          <ol class="sheets">
            {#each plan.order.slice(0, PREVIEWS) as s, i (i)}
              <li>
                <div class="paper sheet static" style="aspect-ratio: {paperRatio}">
                  {#if sheetPreviews[i]}<img src={sheetPreviews[i]} alt="Sheet {i + 1}" />{/if}
                </div>
                <span class="faint small">{s === null ? "Blank back" : `Sheet ${s + 1}`}</span>
              </li>
            {/each}
          </ol>
          {#if plan.order.length > PREVIEWS}
            <p class="faint small more">and {plan.order.length - PREVIEWS} more sheets</p>
          {/if}
        {/if}
      {:else if files.length && sel.length}
        <ol class="sheets">
          {#each sel.slice(0, PREVIEWS) as on, i (i)}
            <li>
              <button class="paper sheet" class:off={!on} style="aspect-ratio: {paperRatio}" aria-pressed={on}
                title={on ? "Click to leave out, shift-click for a run" : "Click to print, shift-click for a run"}
                onclick={(e) => toggle(i, e)}>
                {#if previews[i]}<img src={previews[i]} alt="Page {i + 1}" />{/if}
                <span class="tick" aria-hidden="true">{on ? "✓" : ""}</span>
              </button>
              <span class="faint small">Page {i + 1}</span>
            </li>
          {/each}
        </ol>
        {#if sel.length > PREVIEWS}
          <p class="faint small more">and {sel.length - PREVIEWS} more pages; use Range to pick them</p>
        {/if}
      {/if}
    </div>

    <aside class="pane">
      <section>
        <div class="group-title">Pages</div>
        <div class="props">
          <span class="label">Range</span>
          <input type="text" placeholder="All, or e.g. 1-3, 7" bind:value={ranges}
            oninput={() => { const s = fromRanges(ranges, sel.length); if (s) sel = s; }} />
          <span class="label">Per sheet</span>
          <select bind:value={nup}>
            {#each [1, 2, 4, 6, 9, 16] as n (n)}<option value={n}>{n === 1 ? "1 page" : `${n} pages`}</option>{/each}
          </select>
          <span class="label">Scale</span>
          <div class="seg">
            <button class:on={scaleMode === "Fit"} onclick={() => (scaleMode = "Fit")}>Fit</button>
            <button class:on={scaleMode === "Actual"} onclick={() => (scaleMode = "Actual")} disabled={nup > 1}>100%</button>
            <button class:on={scaleMode === "Percent"} onclick={() => (scaleMode = "Percent")} disabled={nup > 1}>Custom</button>
          </div>
          {#if scaleMode === "Percent" && nup === 1}
            <span class="label">Percent</span>
            <input type="number" min="10" max="400" step="5" bind:value={pct} />
          {/if}
          <span class="label">Pick</span>
          <div class="seg">
            {#each ["All", "Odd", "Even"] as const as v (v)}
              <button class:on={preset === v} onclick={() => pickPreset(v)}>{v}</button>
            {/each}
          </div>
          <span class="label">Order</span>
          <label class="check"><input type="checkbox" bind:checked={reverse} /> Last sheet first</label>
        </div>
      </section>

      <section>
        <div class="group-title">Two-sided</div>
        <label class="check"><input type="checkbox" bind:checked={twoSided} /> Print both sides by hand</label>
        <p class="faint small hint">
          {caps?.duplex ? "This printer can also print two-sided on its own." : "Prints the fronts, then asks you to put the stack back for the backs."}
        </p>
      </section>

      <section>
        <div class="group-title">Copies</div>
        <div class="props">
          <span class="label">Copies</span>
          <input type="number" min="1" max={caps?.max_copies ?? 99} bind:value={copies} />
          <span class="label">Collate</span>
          <label class="check"><input type="checkbox" bind:checked={collate} disabled={copies < 2} /> Whole sets</label>
        </div>
      </section>

      <section>
        <div class="group-title">Paper</div>
        <div class="props">
          <span class="label">Size</span>
          <select bind:value={media}>
            {#each papers as m (m)}<option value={m}>{paperName(m)}</option>{/each}
          </select>
          <span class="label">Colour</span>
          <div class="seg">
            <button class:on={color} onclick={() => (color = true)} disabled={caps ? !caps.color : false}>Colour</button>
            <button class:on={!color} onclick={() => (color = false)}>B&W</button>
          </div>
          <span class="label">Quality</span>
          <div class="seg">
            {#each qualities as q (q)}<button class:on={quality === q} onclick={() => (quality = q)}>{q}</button>{/each}
          </div>
        </div>
      </section>
    </aside>
  </div>
</div>

<style>
  .count { margin-left: 4px; }
  .screen-main { padding: 16px 20px 40px; display: grid; gap: 16px; align-content: start; }
  .files { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: 8px; }
  .files li { display: flex; align-items: center; gap: 2px; height: 34px; padding: 0 6px 0 12px; }
  .files li + li { border-top: 1px solid var(--line); }
  .fname { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .status { font-size: 12px; color: var(--muted); display: flex; gap: 8px; align-items: center; }
  .status.err { color: var(--danger); }
  .status:has(.spinner) { color: var(--text); }
  .flip { border: 1px solid var(--accent); background: var(--accent-soft); border-radius: 8px; padding: 12px 14px; display: grid; gap: 10px; color: var(--text); font-size: 13px; }
  .flip-actions { display: flex; gap: 6px; }
  .drop {
    height: 260px; display: grid; place-content: center; gap: 4px; text-align: center;
    border: 1px dashed var(--line-strong); border-radius: 10px; background: none;
  }
  .screen { position: relative; }
  .dragging::after {
    content: "Drop to add"; position: absolute; inset: 8px; display: grid; place-content: center; pointer-events: none;
    border: 2px dashed var(--accent); border-radius: 12px; background: color-mix(in srgb, var(--bg) 80%, transparent);
    font-weight: 600; z-index: 5;
  }
  .drop:hover:not(:disabled) { background: var(--panel); border-color: var(--accent); }
  .sheets { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 18px 16px; }
  .sheets li { display: grid; gap: 6px; justify-items: center; }
  .sheet {
    width: 100%; height: auto; display: grid; position: relative; padding: 0; border: 0; border-radius: 2px; background: var(--paper);
    outline: 2px solid transparent; outline-offset: 3px; transition: opacity 0.15s var(--ease), outline-color 0.15s var(--ease), transform 0.1s var(--ease);
  }
  .sheet:hover { background: var(--paper); outline-color: var(--line-strong); }
  .sheet.off { opacity: 0.35; }
  .sheet.static { outline: none; }
  .view { justify-self: start; }
  .tick {
    position: absolute; top: 6px; right: 6px; width: 18px; height: 18px; border-radius: 50%; display: grid; place-content: center;
    font-size: 11px; font-weight: 700; color: #fff; background: var(--accent); box-shadow: 0 0 0 1.5px var(--bg);
  }
  .off .tick { background: var(--bg); border: 1.5px solid var(--line-strong); }
  .sheet img { width: 100%; height: 100%; object-fit: contain; display: block; }
  .more { text-align: center; }
  section { display: grid; gap: 8px; }
  .check { display: flex; gap: 8px; align-items: center; font-size: 13px; }
  .hint { margin-top: -2px; }
</style>
