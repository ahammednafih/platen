<script lang="ts">
  import { onMount } from "svelte";
  import { api, DEFAULT_SIZES, type Caps, type ColorMode, type Fill, type Settings } from "./api";

  let { connected }: { connected: boolean } = $props();

  let s = $state<Settings>({ dpi: 300, mode: "Color", fill: "White", size: "A4" });
  // Shown until the device reports its own list.
  let caps = $state<Caps>({ resolutions: [100, 200, 300, 600], modes: ["Color", "Gray", "BlackWhite"], sizes: DEFAULT_SIZES });

  onMount(async () => {
    s = await api.getSettings();
  });

  $effect(() => {
    if (!connected) return;
    api.capabilities().then((c) => {
      if (c.resolutions.length) caps.resolutions = c.resolutions;
      if (c.modes.length) caps.modes = c.modes;
      if (c.sizes.length) caps.sizes = c.sizes;
      if (c.sizes.length && !c.sizes.some((z) => z.id === s.size)) set("size", c.sizes[0].id);
      if (!c.resolutions.includes(s.dpi)) set("dpi", nearest(c.resolutions, s.dpi));
      if (c.modes.length && !c.modes.includes(s.mode)) set("mode", c.modes[0]);
    }).catch(() => {});
  });

  function nearest(list: number[], v: number) {
    return list.reduce((a, b) => (Math.abs(b - v) < Math.abs(a - v) ? b : a), list[0] ?? v);
  }

  function set<K extends keyof Settings>(k: K, v: Settings[K]) {
    s[k] = v;
    api.setSettings($state.snapshot(s));
  }

  const modeLabel: Record<ColorMode, string> = { Color: "Colour", Gray: "Gray", BlackWhite: "B&W" };
  const modeHint: Record<ColorMode, string> = {
    Color: "24-bit colour",
    Gray: "8-bit grayscale",
    BlackWhite: "Pure black and white — smallest files, best for text",
  };
  const fills: [Fill, string][] = [["Off", "Off"], ["White", "White"], ["Paper", "Paper"]];
  const hint = (dpi: number) =>
    dpi <= 100 ? "Draft" : dpi <= 200 ? "Screen" : dpi <= 300 ? "Documents" : dpi <= 600 ? "Photos, small text" : "Archival";
</script>

<section class="settings">
  <div class="group-title">Scan settings</div>
  <div class="field">
    <span class="label">Original size</span>
    <select value={s.size} onchange={(e) => set("size", e.currentTarget.value as Settings["size"])}>
      {#each caps.sizes as z (z.id)}
        <option value={z.id}>{z.name}, {Math.round(z.w_mm)} × {Math.round(z.h_mm)} mm</option>
      {/each}
    </select>
    <span class="faint small">Longer than the glass? Use Long page.</span>
  </div>
  <div class="field">
    <span class="label">Resolution <span class="faint">{hint(s.dpi)}</span></span>
    <div class="seg">
      {#each caps.resolutions as d (d)}
        <button class:on={s.dpi === d} onclick={() => set("dpi", d)} title="{d} dpi">{d}</button>
      {/each}
    </div>
  </div>
  <div class="field">
    <span class="label">Colour</span>
    <div class="seg">
      {#each caps.modes as m (m)}
        <button class:on={s.mode === m} onclick={() => set("mode", m)} title={modeHint[m]}>{modeLabel[m]}</button>
      {/each}
    </div>
  </div>
  <div class="field">
    <span class="label" title="Paint torn or cut-away edges where the dark lid shows through">Fill torn edges</span>
    <div class="seg">
      {#each fills as [v, label] (v)}
        <button class:on={s.fill === v} onclick={() => set("fill", v)}>{label}</button>
      {/each}
    </div>
  </div>
</section>

<style>
  .settings { display: grid; gap: 14px; }
  .field { display: grid; gap: 6px; }
  .label { font-size: 12px; color: var(--muted); display: flex; justify-content: space-between; }
</style>
