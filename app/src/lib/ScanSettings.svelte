<script lang="ts">
  import Seg from "./Seg.svelte";
  import { onMount } from "svelte";
  import { api, DEFAULT_SIZES, type Caps, type ColorMode, type Fill, type Preset, type Settings } from "./api";

  let { connected }: { connected: boolean } = $props();

  let s = $state<Settings>({ dpi: 300, mode: "Color", fill: "White", size: "A4" });
  // Shown until the device reports its own list.
  let caps = $state<Caps>({ resolutions: [100, 200, 300, 600], modes: ["Color", "Gray", "BlackWhite"], sizes: DEFAULT_SIZES });

  let presets = $state<Preset[]>([]);
  let naming = $state<string | null>(null);
  let presetError = $state("");

  onMount(async () => {
    [s, presets] = await Promise.all([api.getSettings(), api.presets()]);
  });

  const same = (a: Settings, b: Settings) => a.dpi === b.dpi && a.mode === b.mode && a.fill === b.fill && a.size === b.size;
  const active = $derived(presets.find((p) => same(p.settings, s))?.name);

  // Values the device doesn't offer fall back to the closest it does.
  function fit(v: Settings): Settings {
    return {
      dpi: caps.resolutions.includes(v.dpi) ? v.dpi : nearest(caps.resolutions, v.dpi),
      mode: caps.modes.includes(v.mode) ? v.mode : caps.modes[0],
      fill: v.fill,
      size: caps.sizes.some((z) => z.id === v.size) ? v.size : caps.sizes[0].id,
    };
  }

  function apply(p: Preset) {
    s = fit(p.settings);
    api.setSettings($state.snapshot(s));
  }

  async function savePreset(e: SubmitEvent) {
    e.preventDefault();
    try {
      presets = await api.savePreset(naming ?? "");
      naming = null;
      presetError = "";
    } catch (err) {
      presetError = String(err);
    }
  }

  async function deletePreset(p: Preset, e: MouseEvent) {
    e.stopPropagation();
    presets = await api.deletePreset(p.name);
  }

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
  <h2 class="head">Scan settings</h2>
  <div class="field">
    <span class="label" id="preset-label">Preset</span>
    <div class="chips" role="group" aria-labelledby="preset-label">
      {#each presets as p (p.name)}
        <span class="chip" class:on={active === p.name}>
          <button aria-pressed={active === p.name} onclick={() => apply(p)}
            title="{p.settings.dpi} dpi, {modeLabel[p.settings.mode]}, fill {p.settings.fill.toLowerCase()}">{p.name}</button>
          {#if !p.builtin}<button class="x" aria-label="Delete preset {p.name}" title="Delete preset" onclick={(e) => deletePreset(p, e)}>×</button>{/if}
        </span>
      {/each}
      {#if naming === null}
        <button class="chip add ghost" onclick={() => (naming = active && !presets.find((p) => p.name === active)?.builtin ? active : "")}
          title="Save the current settings as a preset">+ Save</button>
      {/if}
    </div>
    {#if naming !== null}
      <form class="name" onsubmit={savePreset}>
        <!-- svelte-ignore a11y_autofocus -->
        <input bind:value={naming} aria-label="Preset name" placeholder="Preset name" autofocus onkeydown={(e) => e.key === "Escape" && (naming = null)} />
        <button type="submit">Save</button>
      </form>
    {/if}
    {#if presetError}<span role="alert" class="error small">{presetError}</span>{/if}
  </div>
  <div class="field">
    <label class="label" for="scan-size">Original size</label>
    <select id="scan-size" value={s.size} onchange={(e) => set("size", e.currentTarget.value as Settings["size"])}>
      {#each caps.sizes as z (z.id)}
        <option value={z.id}>{z.name}, {Math.round(z.w_mm)} × {Math.round(z.h_mm)} mm</option>
      {/each}
    </select>
    <span class="faint small">Longer than the glass? Use Long page.</span>
  </div>
  <div class="field">
    <span class="label" id="scan-dpi">Resolution <span class="faint">{hint(s.dpi)}</span></span>
    <Seg aria-labelledby="scan-dpi" value={s.dpi} onchange={(v) => set("dpi", v)} options={caps.resolutions.map((d) => ({ value: d, label: `${d}`, title: `${d} dpi` }))} />
  </div>
  <div class="field">
    <span class="label" id="scan-mode">Colour</span>
    <Seg aria-labelledby="scan-mode" value={s.mode} onchange={(v) => set("mode", v)} options={caps.modes.map((m) => ({ value: m, label: modeLabel[m], title: modeHint[m] }))} />
  </div>
  <div class="field">
    <span class="label" id="scan-fill" title="Paint torn or cut-away edges where the dark lid shows through">Fill torn edges</span>
    <Seg aria-labelledby="scan-fill" value={s.fill} onchange={(v) => set("fill", v)} options={fills.map(([value, label]) => ({ value, label }))} />
  </div>
</section>

<style>
  .settings { display: grid; gap: 14px; }
  /* Same height and rule as the screen's topbar beside it, so the two lines meet. */
  .head { font-size: 0.8125rem; min-height: 44px; display: flex; align-items: center; margin: 0 -16px 2px; padding: 0 16px; border-bottom: 1px solid var(--line); font-weight: 600; }
  .field { display: grid; gap: 6px; }
  .chips { display: flex; flex-wrap: wrap; gap: 4px; }
  .chip { display: inline-flex; align-items: center; min-height: 24px; font-size: 0.75rem; border: 1px solid var(--line-strong); border-radius: var(--radius); background: var(--raised); overflow: hidden; }
  .chip.add { padding: 0 8px; border-color: transparent; background: none; }
  .chip > button { min-height: 22px; padding: 0 8px; font-size: 0.75rem; border: none; border-radius: 0; background: none; }
  .chip > button.x { padding: 0 6px 0 2px; color: var(--muted); }
  .chip > button.x:hover:not(:disabled) { color: var(--text); }
  .chip.on { background: var(--accent-soft); border-color: var(--accent); }
  .chip.on > button:hover:not(:disabled) { background: none; }
  .name { display: flex; gap: 6px; }
  .name input { flex: 1; min-width: 0; }
  .label { font-size: 0.75rem; color: var(--muted); display: flex; justify-content: space-between; }
</style>
