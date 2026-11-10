<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Fill, type Settings } from "./api";

  let s = $state<Settings>({ dpi: 300, color: true, fill: "White" });

  onMount(async () => {
    s = await api.getSettings();
  });

  function set<K extends keyof Settings>(k: K, v: Settings[K]) {
    s[k] = v;
    api.setSettings($state.snapshot(s));
  }

  const dpis = [100, 200, 300, 600];
  const fills: [Fill, string][] = [["Off", "Off"], ["White", "White"], ["Paper", "Paper"]];
</script>

<section class="settings">
  <h3>Scan settings</h3>
  <div class="field">
    <span class="label">Resolution</span>
    <div class="seg">
      {#each dpis as d (d)}
        <button class:on={s.dpi === d} onclick={() => set("dpi", d)}>{d}</button>
      {/each}
    </div>
  </div>
  <div class="field">
    <span class="label">Colour</span>
    <div class="seg">
      <button class:on={s.color} onclick={() => set("color", true)}>Colour</button>
      <button class:on={!s.color} onclick={() => set("color", false)}>Gray</button>
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
  .settings { display: grid; gap: 12px; }
  .field { display: grid; gap: 5px; }
  .label { font-size: 12px; color: var(--muted); }
  .seg { display: flex; background: var(--sunken); border-radius: 8px; padding: 2px; }
  .seg button { flex: 1; border: none; background: none; padding: 4px 0; font-size: 12px; border-radius: 6px; }
  .seg button.on { background: var(--panel); box-shadow: var(--shadow); font-weight: 600; }
</style>
