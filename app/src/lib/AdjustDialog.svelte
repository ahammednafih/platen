<script lang="ts">
  import { img, type Page } from "./api";

  let { page, number, onapply, oncancel }: {
    page: Page;
    number: number;
    onapply: (brightness: number, contrast: number) => void;
    oncancel: () => void;
  } = $props();

  // Percent offsets; the backend applies the same maths as the CSS filter preview.
  let brightness = $state(0);
  let contrast = $state(0);
  const b = $derived(1 + brightness / 100);
  const c = $derived(1 + contrast / 100);
  const changed = $derived(brightness !== 0 || contrast !== 0);

  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") oncancel();
    else if (e.key === "Enter" && changed) onapply(b, c);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop">
  <div class="card dialog" role="dialog" aria-label="Adjust page {number}">
    <h2>Adjust page {number}</h2>
    <div class="view">
      <img src={img("pages", page.file, 1200)} alt="Page {number}" style:filter="brightness({b}) contrast({c})" />
    </div>
    <label class="slider">
      <span>Brightness</span>
      <input type="range" min="-50" max="50" bind:value={brightness} />
      <span class="val">{brightness > 0 ? "+" : ""}{brightness}%</span>
    </label>
    <label class="slider">
      <span>Contrast</span>
      <input type="range" min="-50" max="80" bind:value={contrast} />
      <span class="val">{contrast > 0 ? "+" : ""}{contrast}%</span>
    </label>
    <footer>
      <button class="ghost" onclick={() => (brightness = contrast = 0)} disabled={!changed}>Reset</button>
      <span class="grow"></span>
      <button class="ghost" onclick={oncancel}>Cancel</button>
      <button class="primary" onclick={() => onapply(b, c)} disabled={!changed}>Apply</button>
    </footer>
  </div>
</div>

<style>
  .dialog { width: min(560px, 92vw); padding: 20px 22px; display: grid; gap: 12px; }
  h2 { margin: 0; }
  .view { height: min(56vh, 600px); display: grid; place-items: center; background: var(--panel); border-radius: 6px; padding: 10px; }
  .view img { max-width: 100%; max-height: 100%; object-fit: contain; background: #fff; box-shadow: 0 0 0 1px var(--line-strong); }
  .slider { display: grid; grid-template-columns: 80px 1fr 48px; gap: 10px; align-items: center; }
  .slider input { accent-color: var(--accent); }
  .val { text-align: right; font-variant-numeric: tabular-nums; color: var(--muted); }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
</style>
