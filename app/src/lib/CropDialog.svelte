<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "./Modal.svelte";
  import { api, img, mm, type Page } from "./api";

  let { page, index, onapply, onclose }: {
    page: Page;
    index: number;
    onapply: (r: { x: number; y: number; w: number; h: number }) => void;
    onclose: () => void;
  } = $props();

  // The crop rectangle in fractions of the page, from the top-left.
  let r = $state({ x: 0, y: 0, w: 1, h: 1 });
  let picked = $state("Whole page");
  let note = $state("");
  let box = $state<HTMLElement | null>(null);

  const [pageW, pageH] = $derived(mm(page));
  // Originals sit at the glass's top-left corner, so sizes are measured from there.
  const sizes: [string, number, number][] = [
    ["A4", 210, 297], ["A5", 148, 210], ["A6", 105, 148], ["B5", 176, 250], ["Letter", 216, 279],
    ["Photo 10×15", 102, 152], ["Card", 86, 54],
  ];
  const fits = (w: number, h: number) => w <= pageW + 2 && h <= pageH + 2;

  function whole() {
    r = { x: 0, y: 0, w: 1, h: 1 };
    picked = "Whole page";
    note = "";
  }

  async function auto() {
    picked = "Auto";
    note = "";
    const b = await api.paperBounds(index).catch(() => null);
    if (b) r = { x: b[0], y: b[1], w: b[2], h: b[3] };
    else note = "No dark border around the paper to find. Pick a size or drag a box.";
  }

  function size(name: string, w: number, h: number) {
    picked = name;
    note = "";
    r = { x: 0, y: 0, w: Math.min(1, w / pageW), h: Math.min(1, h / pageH) };
  }

  // Drag on empty space draws a new box; drag inside moves it; drag a corner resizes it.
  type Drag = { kind: "new" | "move" | "nw" | "ne" | "sw" | "se"; sx: number; sy: number; start: typeof r };
  let drag: Drag | null = null;
  const clamp = (v: number) => Math.min(1, Math.max(0, v));

  function at(e: PointerEvent) {
    const b = box!.getBoundingClientRect();
    return { x: clamp((e.clientX - b.left) / b.width), y: clamp((e.clientY - b.top) / b.height) };
  }

  function down(e: PointerEvent, kind: Drag["kind"]) {
    e.preventDefault();
    e.stopPropagation();
    const p = at(e);
    drag = { kind, sx: p.x, sy: p.y, start: { ...r } };
    box!.setPointerCapture(e.pointerId);
    picked = "Custom";
    note = "";
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const p = at(e);
    const s = drag.start;
    if (drag.kind === "move") {
      r = { ...s, x: Math.min(1 - s.w, Math.max(0, s.x + p.x - drag.sx)), y: Math.min(1 - s.h, Math.max(0, s.y + p.y - drag.sy)) };
      return;
    }
    // The corner opposite the one being dragged stays put.
    const fx = drag.kind === "new" ? drag.sx : drag.kind.endsWith("w") ? s.x + s.w : s.x;
    const fy = drag.kind === "new" ? drag.sy : drag.kind.startsWith("n") ? s.y + s.h : s.y;
    r = { x: Math.min(fx, p.x), y: Math.min(fy, p.y), w: Math.abs(p.x - fx), h: Math.abs(p.y - fy) };
  }

  function up() {
    drag = null;
    if (r.w < 0.02 || r.h < 0.02) whole();
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === "Enter") apply();
    else return;
    e.preventDefault();
  }

  const isWhole = $derived(r.x < 0.002 && r.y < 0.002 && r.w > 0.998 && r.h > 0.998);
  function apply() {
    if (!isWhole) onapply(r);
    else onclose();
  }

  onMount(auto);
</script>

<svelte:window onkeydown={onkey} />

<Modal label="Crop page {index + 1}" {onclose}>
  <div class="card dialog">
    <h2>Crop page {index + 1}</h2>
    <div class="body">
      <div class="frame"><div class="view">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="page" bind:this={box} onpointerdown={(e) => down(e, "new")} onpointermove={move} onpointerup={up}>
          <img src={img("pages", page.file, 1200)} alt="Page {index + 1}" draggable="false" />
          <div class="keep" style="left:{r.x * 100}%; top:{r.y * 100}%; width:{r.w * 100}%; height:{r.h * 100}%" onpointerdown={(e) => down(e, "move")}>
            {#each ["nw", "ne", "sw", "se"] as const as c (c)}
              <span class="handle {c}" onpointerdown={(e) => down(e, c)}></span>
            {/each}
          </div>
        </div>
      </div></div>
      <aside>
        <div class="group-title">Crop to</div>
        <div class="choices">
          <button class:on={picked === "Auto"} onclick={auto} title="Find the paper from the dark border around it">Auto</button>
          {#each sizes.filter(([, w, h]) => fits(w, h)) as [name, w, h] (name)}
            <button class:on={picked === name} onclick={() => size(name, w, h)}>{name} <span class="faint">{w}×{h}</span></button>
          {/each}
          <button class:on={picked === "Whole page"} onclick={whole}>Whole page</button>
        </div>
        <p class="muted small">Or drag on the page to draw a box. Drag inside it to move, or a corner to resize.</p>
        <p class="small result">{Math.round(r.w * pageW)} × {Math.round(r.h * pageH)} mm</p>
        {#if note}<p class="warn small">{note}</p>{/if}
      </aside>
    </div>
    <footer>
      <button class="ghost" onclick={onclose}>Cancel</button>
      <span class="grow"></span>
      <button class="primary" onclick={apply} disabled={isWhole}>Crop</button>
    </footer>
  </div>
</Modal>

<style>
  .dialog { width: min(820px, 94vw); padding: 20px 22px; display: grid; gap: 14px; }
  .body { display: grid; grid-template-columns: minmax(0, 1fr) 190px; gap: 18px; }
  .frame { background: var(--sunken); border-radius: 8px; padding: 12px; }
  /* The image sizes the page box (portrait or landscape), bounded by the view via container units. */
  .view { height: min(62vh, 640px); container-type: size; display: grid; place-items: center; }
  .page { position: relative; line-height: 0; touch-action: none; cursor: crosshair; background: var(--paper); box-shadow: 0 0 0 1px var(--line-strong); }
  .page img { max-width: 100cqw; max-height: 100cqh; display: block; pointer-events: none; }
  /* Everything outside the box is dimmed by the box's own huge shadow. */
  .keep { position: absolute; cursor: move; outline: 1.5px solid var(--accent); box-shadow: 0 0 0 9999px rgb(0 0 0 / 45%); }
  .page { overflow: hidden; }
  .handle { position: absolute; width: 12px; height: 12px; background: #fff; border: 1.5px solid var(--accent); border-radius: 50%; }
  .nw { left: -6px; top: -6px; cursor: nwse-resize; }
  .se { right: -6px; bottom: -6px; cursor: nwse-resize; }
  .ne { right: -6px; top: -6px; cursor: nesw-resize; }
  .sw { left: -6px; bottom: -6px; cursor: nesw-resize; }
  aside { display: grid; gap: 10px; align-content: start; }
  .choices { display: grid; gap: 4px; }
  .choices button { justify-content: space-between; height: 26px; font-size: 12px; }
  .choices button.on { border-color: var(--accent); background: var(--accent-soft); }
  .result { font-variant-numeric: tabular-nums; font-weight: 600; }
  .warn { color: var(--warn); }
  .small { font-size: 12px; }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
</style>
