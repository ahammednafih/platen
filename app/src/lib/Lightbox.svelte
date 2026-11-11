<script lang="ts">
  import { api, img, mm, type Page, type PageText } from "./api";

  let { pages, index = $bindable(), onclose }: { pages: Page[]; index: number; onclose: () => void } = $props();

  const page = $derived(pages[index]);

  // Text mode: OCR the page once, then lay selectable words over the image.
  let textMode = $state(false);
  let texts = $state<Record<string, PageText>>({});
  let reading = $state(false);
  let error = $state("");
  let copied = $state(false);
  const text = $derived(page ? texts[page.file] : undefined);

  $effect(() => {
    if (textMode && page && !texts[page.file]) read(page.file);
  });

  async function read(file: string) {
    reading = true;
    error = "";
    try {
      texts[file] = await api.pageText(file);
    } catch (e) {
      error = String(e);
    } finally {
      reading = false;
    }
  }

  async function copyAll() {
    if (!text?.text) return;
    await navigator.clipboard.writeText(text.text);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  // Zoom: 1 is fit-to-window. The fitted size is measured once, then scaled, so the page scrolls inside the stage.
  let zoom = $state(1);
  let base = $state<{ w: number } | null>(null);
  let image = $state<HTMLImageElement | null>(null);
  let pinchFrom = 1;

  function setZoom(z: number) {
    if (zoom === 1 && image) base = { w: image.getBoundingClientRect().width };
    zoom = Math.min(8, Math.max(1, Math.round(z * 100) / 100));
  }
  $effect(() => {
    index;
    zoom = 1;
  });

  // Trackpad pinch arrives as ctrl+wheel in some webviews and as gesture events in WebKit.
  function wheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    setZoom(zoom * Math.exp(-e.deltaY / 100));
  }
  let stage = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!stage) return;
    const start = (e: Event) => { e.preventDefault(); pinchFrom = zoom; };
    const change = (e: Event) => { e.preventDefault(); setZoom(pinchFrom * (e as Event & { scale: number }).scale); };
    stage.addEventListener("gesturestart", start);
    stage.addEventListener("gesturechange", change);
    return () => { stage?.removeEventListener("gesturestart", start); stage?.removeEventListener("gesturechange", change); };
  });

  const selecting = () => !!window.getSelection()?.toString();

  function onkey(e: KeyboardEvent) {
    if (e.metaKey) return; // leave ⌘C to the browser
    if (e.key === "Escape" || (e.key === " " && !textMode)) onclose();
    else if (e.key === "+" || e.key === "=") setZoom(zoom * 1.5);
    else if (e.key === "-") setZoom(zoom / 1.5);
    else if (e.key === "0") setZoom(1);
    else if (e.key === "t" || e.key === "T") textMode = !textMode;
    else if (zoom > 1 && e.key.startsWith("Arrow")) return; // arrows scroll the zoomed page
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") index = Math.min(pages.length - 1, index + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") index = Math.max(0, index - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

{#if page}
  <div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget && !selecting()) onclose(); }}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="stage" class:zoomed={zoom > 1} bind:this={stage} onwheel={wheel}
      onclick={(e) => { if (e.target === e.currentTarget && !selecting()) onclose(); }} onkeydown={() => {}}>
    <div class="frame">
      <img bind:this={image} src={img("pages", page.file, zoom > 1.5 ? 3600 : 1800)} alt="Page {index + 1}"
        style={zoom > 1 && base ? `width:${base.w * zoom}px; max-width:none; max-height:none` : ""} />
      {#if textMode && text}
        <svg class="words" viewBox="0 0 1000 {(1000 * page.h) / page.w}" preserveAspectRatio="none">
          {#each text.lines as line, i (i)}
            {#each line as w, j (j)}
              {@const k = 1000}
              <rect class="hint" x={w.x * k} y={(w.y * k * page.h) / page.w} width={w.w * k} height={(w.h * k * page.h) / page.w} rx="2" />
              <text x={w.x * k} y={((w.y + w.h * 0.82) * k * page.h) / page.w} font-size={(w.h * k * page.h) / page.w} textLength={w.w * k} lengthAdjust="spacingAndGlyphs">{w.text}{j < line.length - 1 ? " " : "\n"}</text>
            {/each}
          {/each}
        </svg>
      {/if}
    </div>
    </div>
    <div class="bar">
      <span class="caption">Page {index + 1} of {pages.length}, {mm(page).join(" × ")} mm, {page.dpi} dpi</span>
      <span class="zoom">
        <button class="ghost small" onclick={() => setZoom(zoom / 1.5)} disabled={zoom <= 1} title="Zoom out (−)" aria-label="Zoom out">−</button>
        <button class="ghost small pct" onclick={() => setZoom(1)} title="Fit to window (0)">{zoom === 1 ? "Fit" : `${Math.round(zoom * 100)}%`}</button>
        <button class="ghost small" onclick={() => setZoom(zoom * 1.5)} disabled={zoom >= 8} title="Zoom in (+), or pinch" aria-label="Zoom in">+</button>
      </span>
      <button class="ghost small" class:on={textMode} onclick={() => (textMode = !textMode)} title="Select text on the page (T)">
        {#if reading}<span class="spinner"></span>Reading text{:else}Text{/if}
      </button>
      {#if textMode && text}
        <button class="ghost small" onclick={copyAll} disabled={!text.text}>{copied ? "Copied" : text.text ? "Copy all text" : "No text found"}</button>
      {/if}
      {#if error}<span class="err">{error}</span>{/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 20; background: rgb(10 10 10 / 88%);
    display: grid; grid-template-rows: 1fr auto; place-items: center; padding: 24px; gap: 12px;
  }
  /* The frame centres with auto margins, so a zoomed page scrolls to every edge instead of clipping. */
  .stage { display: flex; width: 100%; height: 100%; min-height: 0; overflow: auto; }
  .frame { position: relative; display: inline-block; line-height: 0; margin: auto; flex: none; max-width: 100%; max-height: calc(100vh - 90px); }
  .zoomed .frame { max-width: none; max-height: none; }
  .zoom { display: flex; align-items: center; gap: 1px; }
  .zoom .pct { min-width: 48px; font-variant-numeric: tabular-nums; }
  .zoom button { font-size: 14px; }
  img { max-width: 100%; max-height: calc(100vh - 90px); object-fit: contain; box-shadow: 0 8px 40px rgb(0 0 0 / 50%); background: #fff; }
  .words { position: absolute; inset: 0; width: 100%; height: 100%; user-select: text; -webkit-user-select: text; cursor: text; }
  .words text { fill: transparent; white-space: pre; font-family: -apple-system, system-ui, sans-serif; }
  .words text::selection { fill: transparent; background: rgb(64 132 255 / 40%); }
  .words .hint { fill: rgb(64 132 255 / 10%); pointer-events: none; }
  .bar { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; justify-content: center; }
  .caption { color: #ddd; font-size: 13px; }
  .bar button { color: #ddd; }
  .bar button:hover:not(:disabled) { background: rgb(255 255 255 / 10%); color: #fff; }
  .bar button.on { background: rgb(255 255 255 / 16%); color: #fff; }
  .err { color: #f0a59c; font-size: 12px; }
</style>
