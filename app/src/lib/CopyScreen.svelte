<script lang="ts">
  import { onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api, covered, DEFAULT_SIZES, fileImg, paperName, type PrintCaps, type PrintJob, type Quality, type Scale, type Size, type SizeId, type TopHalf } from "./api";

  let { connected }: { connected: boolean } = $props();

  type Kind = "copy" | "twoInOne" | "idCard" | "long";
  type Step = { prompt: string; hint: string; button: string };

  const kinds: { id: Kind; name: string; about: string; steps: Step[] }[] = [
    {
      id: "copy", name: "Copy", about: "One page onto one sheet.",
      steps: [{ prompt: "Put the page face down on the glass, top edge against the far edge.", hint: "", button: "Copy" }],
    },
    {
      id: "twoInOne", name: "2 in 1", about: "Two pages side by side on one sheet.",
      steps: [
        { prompt: "Put the first page face down on the glass.", hint: "It goes on the left of the sheet.", button: "Scan first page" },
        { prompt: "Put the second page face down on the glass.", hint: "The copy prints right after this scan.", button: "Scan second page" },
      ],
    },
    {
      id: "idCard", name: "ID card", about: "Front and back of a card on one sheet, at actual size.",
      steps: [
        { prompt: "Put the card face down in the top half of the glass, a little away from the corner.", hint: "Anywhere in the top half works.", button: "Scan front" },
        { prompt: "Turn the card over and put it back in the same spot.", hint: "The copy prints right after this scan.", button: "Scan back" },
      ],
    },
    {
      id: "long", name: "Long page", about: "A page taller than the glass, joined from two scans and shrunk onto one sheet.",
      steps: [
        { prompt: "Put the top of the page face down on the glass, top edge against the far edge.", hint: "The rest of the sheet can hang off the bottom.", button: "Scan top half" },
        { prompt: "Turn the page around so the bottom is on the glass, bottom edge against the far edge.", hint: "Leave a few centimetres of overlap with the top scan.", button: "Scan bottom half" },
      ],
    },
  ];

  let kind = $state<Kind>("copy");
  let sizes = $state<Size[]>(DEFAULT_SIZES);
  let original = $state<SizeId>("A4");
  let caps = $state<PrintCaps | null>(null);
  let copies = $state(1);
  let color = $state(false);
  let scaleMode = $state<"Fit" | "Actual" | "Percent">("Actual");
  let pct = $state(100);
  let media = $state("iso_a4_210x297mm");
  let quality = $state<Quality>("Normal");
  let density = $state(0);
  let background = $state(0);

  let step = $state(0);
  let scans = $state<string[]>([]);
  let top = $state<TopHalf | null>(null);
  let busy = $state("");
  let log = $state<{ ok: boolean; text: string } | null>(null);

  const k = $derived(kinds.find((x) => x.id === kind)!);
  const papers = $derived((caps?.media ?? ["iso_a4_210x297mm", "na_letter_8.5x11in"]).filter((m) => !m.startsWith("custom_")));
  const qualities = $derived<Quality[]>(caps?.qualities.length ? caps.qualities : ["Draft", "Normal", "High"]);
  /** Paper size in mm from an IPP keyword like `iso_a4_210x297mm` or `na_letter_8.5x11in`. */
  const paperMm = (kw: string) => {
    const m = kw.match(/([\d.]+)x([\d.]+)(mm|in)$/);
    const k = m?.[3] === "in" ? 25.4 : 1;
    return m ? [Number(m[1]) * k, Number(m[2]) * k] : [210, 297];
  };
  const fitPct = $derived.by(() => {
    const z = sizes.find((x) => x.id === original);
    if (!z) return 0;
    const [w, h] = paperMm(media);
    return Math.floor(Math.min(w / z.w_mm, h / z.h_mm) * 100);
  });
  const scale = $derived<Scale>(kind === "copy" ? (scaleMode === "Percent" ? { Percent: pct } : scaleMode) : kind === "idCard" ? "Actual" : "Fit");

  $effect(() => {
    if (!connected) return;
    api.capabilities().then((c) => {
      if (c.sizes.length) sizes = c.sizes;
      if (c.sizes.length && !c.sizes.some((z) => z.id === original)) original = c.sizes[0].id;
    }).catch(() => {});
    api.printCaps().then((c) => {
      caps = c;
      if (!c.media.includes(media)) media = c.media_default;
    }).catch(() => {});
  });

  const unlisten = listen<{ done: number; total: number }>("print-progress", (e) => {
    if (busy && !stopping) busy = `Printing ${e.payload.done}/${e.payload.total}`;
  });
  onDestroy(() => {
    unlisten.then((f) => f());
    api.copyReset();
  });

  async function reset() {
    step = 0;
    scans = [];
    top = null;
    await api.copyReset();
  }

  function choose(id: Kind) {
    if (busy) return;
    kind = id;
    log = null;
    reset();
  }

  async function scanStep(): Promise<string | null> {
    if (kind === "long") {
      if (step === 0) {
        top = await api.copyLongTop(color);
        return top.path;
      }
      return api.copyLongBottom(top!, color);
    }
    return api.copyScan(kind === "idCard" ? "HalfTop" : { Size: original }, color);
  }

  let stopping = $state(false);
  async function stop() {
    stopping = true;
    busy = "Stopping";
    await api.stopJob().catch(() => {});
  }
  const failed = (e: unknown) => (String(e) === "stopped" ? { ok: true, text: "Stopped. Nothing else will print." } : { ok: false, text: String(e) });

  async function go() {
    log = null;
    busy = "Scanning";
    try {
      const path = await scanStep();
      if (path) scans = [...scans, path];
      if (step < k.steps.length - 1) {
        step++;
        return;
      }
      let files = kind === "long" ? [scans[scans.length - 1]] : scans;
      if (kind === "idCard") {
        busy = "Placing front and back";
        files = [await api.copyIdCard(scans[0], scans[1])];
      }
      if (density || background) {
        busy = "Adjusting";
        files = await api.copyTone(files, density, background);
      }
      busy = "Printing";
      const job: PrintJob = {
        files, ranges: "", subset: "All", reverse: false, nup: kind === "twoInOne" ? 2 : 1, scale, collate: true,
        opts: { copies, color, media, scaling: "fit", quality },
      };
      await api.print(job);
      log = { ok: true, text: `Sent ${copies} ${copies === 1 ? "copy" : "copies"} to the printer.` };
      await reset();
    } catch (e) {
      log = failed(e);
    } finally {
      busy = "";
      stopping = false;
    }
  }
  function onkey(e: KeyboardEvent) {
    if (!e.metaKey || e.key !== "Enter" || !connected || busy || covered()) return;
    e.preventDefault();
    go();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="screen">
  <header class="topbar">
    <h1>Copy</h1>
    <span class="grow"></span>
    {#if step > 0}
      <button class="ghost" onclick={reset} disabled={!!busy}>Start over</button>
    {/if}
  </header>

  <div class="screen-body">
    <div class="screen-main">
      <div class="kinds" role="tablist">
        {#each kinds as x (x.id)}
          <button role="tab" aria-selected={kind === x.id} class:on={kind === x.id} onclick={() => choose(x.id)} disabled={!!busy && kind !== x.id}>
            <span class="glyph {x.id}" aria-hidden="true"><i></i><i></i></span>
            <span class="kname">{x.name}</span>
            <span class="kabout">{x.about}</span>
          </button>
        {/each}
      </div>

      <div class="stage">
        <svg class="glass {kind}" class:second={step > 0} viewBox="0 0 120 150" aria-hidden="true">
          <rect class="bed" x="4" y="4" width="112" height="142" rx="6" />
          <rect class="edge" x="4" y="4" width="112" height="6" rx="3" />
          <rect class="sheet" x="10" y="12" />
        </svg>
        <div class="stage-text">
        {#if k.steps.length > 1}
          <ol class="steps">
            {#each k.steps as s, i (i)}
              <li class:on={i === step} class:done={i < step}>{s.button.replace(/^Scan /, "")}</li>
            {/each}
          </ol>
        {/if}
        <p class="prompt">{k.steps[step].prompt}</p>
        {#if k.steps[step].hint}<p class="muted">{k.steps[step].hint}</p>{/if}
        <div class="go">
          <button class="primary big" onclick={go} disabled={!connected || !!busy}>{k.steps[step].button}</button>
        </div>

        {#if busy}
          <p class="status"><span class="spinner"></span>{busy}… <button class="ghost small stop" onclick={stop} disabled={stopping}>Stop</button></p>
        {:else if log}
          <p class="status" class:err={!log.ok}>{log.text}</p>
        {:else if !connected}
          <p class="status">Waiting for the scanner.</p>
        {/if}

        </div>
        {#if scans.length}
          <div class="scans">
            {#each scans as s, i (s)}
              <figure>
                <div class="paper"><img src={fileImg(s, 0, 300)} alt="Scan {i + 1}" /></div>
                <figcaption class="faint small">{k.steps[i]?.button.replace(/^Scan /, "") ?? ""}</figcaption>
              </figure>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <aside class="pane">
      <section>
        <div class="group-title">Copy settings</div>
        <div class="props">
          {#if kind === "copy" || kind === "twoInOne"}
            <span class="label">Original</span>
            <select bind:value={original}>
              {#each sizes as z (z.id)}<option value={z.id}>{z.name}</option>{/each}
            </select>
          {/if}
          <span class="label">Copies</span>
          <input type="number" min="1" max={caps?.max_copies ?? 99} bind:value={copies} />
          <span class="label">Colour</span>
          <div class="seg">
            <button class:on={color} onclick={() => (color = true)} disabled={caps ? !caps.color : false}>Colour</button>
            <button class:on={!color} onclick={() => (color = false)}>B&W</button>
          </div>
          {#if kind === "copy"}
            <span class="label">Scale</span>
            <div class="seg">
              <button class:on={scaleMode === "Actual"} onclick={() => (scaleMode = "Actual")}>100%</button>
              <button class:on={scaleMode === "Fit"} onclick={() => (scaleMode = "Fit")} title={fitPct ? `Fit the original to the paper (${fitPct}%)` : "Fit to paper"}>Fit</button>
              <button class:on={scaleMode === "Percent"} onclick={() => (scaleMode = "Percent")}>Custom</button>
            </div>
            {#if scaleMode === "Percent"}
              <span class="label">Percent</span>
              <input type="number" min="25" max="400" step="5" bind:value={pct} />
            {/if}
          {/if}
          <span class="label">Density</span>
          <select bind:value={density}>
            {#each [-3, -2, -1, 0, 1, 2, 3] as d (d)}<option value={d}>{d > 0 ? `+${d} darker` : d < 0 ? `${d} lighter` : "0"}</option>{/each}
          </select>
          <span class="label">Background</span>
          <select bind:value={background} title="Push tinted or grey paper to white">
            {#each ["Keep", "Remove (low)", "Remove (medium)", "Remove (high)"] as b, i (b)}<option value={i}>{b}</option>{/each}
          </select>
        </div>
      </section>
      <section>
        <div class="group-title">Paper</div>
        <div class="props">
          <span class="label">Size</span>
          <select bind:value={media}>
            {#each papers as m (m)}<option value={m}>{paperName(m)}</option>{/each}
          </select>
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
  .stage { display: grid; grid-template-columns: 120px minmax(0, 1fr); gap: 24px; align-items: start; padding: 20px; border: 1px solid var(--line); border-radius: 12px; background: var(--panel); }
  .stage-text { display: grid; gap: 10px; align-content: start; min-width: 0; }
  .stage .prompt { font-size: 15px; font-weight: 600; letter-spacing: -0.01em; text-wrap: balance; }
  .go { display: flex; gap: 8px; margin-top: 4px; }
  button.big { height: 34px; padding: 0 18px; font-weight: 600; }
  .glass { width: 120px; height: 150px; }
  .glass .bed { fill: var(--sunken); stroke: var(--line-strong); }
  .glass .edge { fill: var(--line-strong); }
  .glass .sheet { fill: var(--paper); stroke: var(--accent); stroke-width: 1.5; width: 100px; height: 128px; rx: 2; transition: all 0.3s var(--ease); }
  .glass.idCard .sheet { width: 54px; height: 34px; }
  .glass.long .sheet { height: 150px; stroke-dasharray: 4 3; }
  .glass.twoInOne.second .sheet, .glass.idCard.second .sheet { fill: var(--accent-soft); }
  @media (max-width: 720px) { .stage { grid-template-columns: minmax(0, 1fr); } .glass { display: none; } }
  .screen-main { padding: 16px 20px 40px; display: grid; gap: 28px; align-content: start; }
  .kinds { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 8px; }
  .kinds button {
    height: auto; padding: 12px; display: grid; justify-items: start; align-content: start; gap: 4px; text-align: left;
    white-space: normal; background: var(--panel); border-color: var(--line); border-radius: 8px;
  }
  .kinds button.on { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); background: var(--accent-soft); }
  .kname { font-weight: 600; }
  .kabout { font-size: 12px; color: var(--muted); line-height: 1.4; }

  /* Small sheet glyph showing where the copy lands on the paper. */
  .glyph { width: 26px; height: 36px; margin-bottom: 6px; position: relative; background: var(--paper); border-radius: 2px; box-shadow: 0 0 0 1px var(--line-strong); }
  .glyph i { position: absolute; background: var(--faint); border-radius: 1px; }
  .on .glyph i { background: var(--accent); }
  .copy i:first-child { inset: 4px; }
  .copy i:last-child { display: none; }
  .twoInOne i:first-child { left: 4px; right: 4px; top: 4px; height: 12px; }
  .twoInOne i:last-child { left: 4px; right: 4px; bottom: 4px; height: 12px; }
  .idCard i:first-child { left: 4px; top: 4px; width: 13px; height: 8px; }
  .idCard i:last-child { left: 4px; top: 20px; width: 13px; height: 8px; }
  .long i:first-child { left: 8px; right: 8px; top: 3px; bottom: 3px; }
  .long i:last-child { display: none; }

  .stage { max-width: 560px; display: grid; gap: 8px; }
  .steps { list-style: none; margin: 0 0 6px; padding: 0; display: flex; gap: 6px; counter-reset: step; }
  .steps li { counter-increment: step; font-size: 12px; color: var(--faint); display: flex; align-items: center; gap: 6px; text-transform: capitalize; }
  .steps li::before { content: counter(step); width: 18px; height: 18px; border-radius: 50%; display: grid; place-items: center; border: 1px solid var(--line-strong); font-size: 11px; }
  .steps li + li::after { order: -1; content: ""; width: 18px; height: 1px; background: var(--line-strong); }
  .steps li.on { color: var(--text); }
  .steps li.on::before { background: var(--accent); border-color: var(--accent); color: var(--accent-text); }
  .steps li.done::before { content: "✓"; color: var(--ok); border-color: var(--ok); }
  .prompt { font-size: 15px; line-height: 1.45; font-weight: 500; }
  .status { margin-top: 6px; font-size: 12px; color: var(--muted); display: flex; align-items: center; gap: 8px; }
  .status.err { color: var(--danger); }
  .status:has(.spinner) { color: var(--text); }
  .scans { display: flex; gap: 14px; margin-top: 10px; }
  figure { margin: 0; display: grid; gap: 6px; justify-items: center; }
  figure .paper { width: 120px; }
  figure img { width: 100%; display: block; }
  figcaption { text-transform: capitalize; }
  section { display: grid; gap: 8px; }
</style>
