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

  const selecting = () => !!window.getSelection()?.toString();

  function onkey(e: KeyboardEvent) {
    if (e.metaKey) return; // leave ⌘C to the browser
    if (e.key === "Escape" || (e.key === " " && !textMode)) onclose();
    else if (e.key === "t" || e.key === "T") textMode = !textMode;
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") index = Math.min(pages.length - 1, index + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") index = Math.max(0, index - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

{#if page}
  <div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget && !selecting()) onclose(); }}>
    <div class="frame">
      <img src={img("pages", page.file, 1800)} alt="Page {index + 1}" />
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
    <div class="bar">
      <span class="caption">Page {index + 1} of {pages.length}, {mm(page).join(" × ")} mm, {page.dpi} dpi</span>
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
  .frame { position: relative; display: inline-block; line-height: 0; max-width: 100%; max-height: calc(100vh - 90px); }
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
