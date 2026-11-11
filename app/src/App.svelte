<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import CopyScreen from "./lib/CopyScreen.svelte";
  import DevicePanel from "./lib/DevicePanel.svelte";
  import ExportDialog from "./lib/ExportDialog.svelte";
  import Library from "./lib/Library.svelte";
  import PrintScreen from "./lib/PrintScreen.svelte";
  import ScanScreen from "./lib/ScanScreen.svelte";
  import ScanSettings from "./lib/ScanSettings.svelte";
  import Shortcuts from "./lib/Shortcuts.svelte";
  import Start from "./lib/Start.svelte";
  import { api, type SessionView } from "./lib/api";

  type View = "scan" | "copy" | "print" | "library";

  let connected = $state(false);
  let session = $state<SessionView | null>(null);
  let exporting = $state(false);
  let view = $state<View>("scan");
  let help = $state(false);

  type Theme = "system" | "light" | "dark";
  const stored = (): Theme => { try { return (localStorage.getItem("theme") as Theme) || "system"; } catch { return "system"; } };
  let theme = $state<Theme>(stored());
  const accents = [
    ["ink", "Ink black", "#1c1d1f", "#e8e6e1"], ["magenta", "Magenta", "#c2185b", "#e0457f"], ["orange", "Toner orange", "#d9480f", "#f2703a"],
    ["teal", "Teal", "#0f766e", "#2bb3a3"], ["coffee", "Coffee", "#7a4a2a", "#8b5a3c"],
  ] as const;
  type Accent = (typeof accents)[number][0];
  let accent = $state<Accent>((() => { try { return (localStorage.getItem("accent") as Accent) || "ink"; } catch { return "ink"; } })());
  $effect(() => {
    if (accent === "ink") delete document.documentElement.dataset.accent;
    else document.documentElement.dataset.accent = accent;
    try { localStorage.setItem("accent", accent); } catch {}
  });
  $effect(() => {
    if (theme === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
    try { localStorage.setItem("theme", theme); } catch {}
    getCurrentWindow().setTheme(theme === "system" ? null : theme).catch(() => {});
  });

  // 16px line icons, stroke drawn in currentColor.
  const icons: Record<View, string> = {
    scan: "M4 2.5h5.5L12 5v8.5H4z M9.5 2.5V5H12 M6 8h4 M6 10.5h4",
    copy: "M5.5 4.5V2.5h6L14 5v7h-2.5 M2.5 4.5h6L11 7v6.5H2.5z",
    print: "M4.5 6V2.5h7V6 M4.5 11.5h-2v-5.5h11v5.5h-2 M4.5 9.5h7v4h-7z",
    library: "M2.5 4.5v8.5h11V6H8L6.5 4.5z",
  };
  const nav: [View, string, string][] = [
    ["scan", "Scan", "1"],
    ["copy", "Copy", "2"],
    ["print", "Print", "3"],
    ["library", "Saved scans", "4"],
  ];

  async function close() {
    await api.closeSession();
    session = null;
  }

  function onkey(e: KeyboardEvent) {
    if (!e.metaKey || exporting) return;
    if (e.key === "/") {
      help = !help;
      return e.preventDefault();
    }
    const hit = nav.find(([, , k]) => k === e.key);
    if (!hit) return;
    view = hit[0];
    e.preventDefault();
  }
</script>

<!-- Stop files dropped outside a drop target from replacing the app with the file. -->
<svelte:window onkeydown={onkey} ondragover={(e) => e.preventDefault()} ondrop={(e) => e.preventDefault()} />

<div class="shell">
  <aside class="side">
    <div class="brand">Scan</div>
    <nav>
      {#each nav as [v, label, key] (v)}
        <button class:on={view === v} onclick={() => (view = v)} title="{label} (⌘{key})">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"><path d={icons[v]} /></svg>
          {label}
        </button>
      {/each}
    </nav>
    <div class="grow"></div>
    <div class="seg theme" title="Appearance">
      {#each [["system", "System"], ["light", "Light"], ["dark", "Dark"]] as [v, label] (v)}
        <button class:on={theme === v} onclick={() => (theme = v as Theme)}>{label}</button>
      {/each}
    </div>
    <div class="accents" role="radiogroup" aria-label="Accent colour">
      {#each accents as [v, label, light, dark] (v)}
        <button role="radio" aria-checked={accent === v} class:on={accent === v} title={label} aria-label={label} style:--light={light} style:--dark={dark} onclick={() => (accent = v)}></button>
      {/each}
    </div>
    <button class="ghost keys" onclick={() => (help = true)}>Keyboard shortcuts <span class="faint">⌘/</span></button>
    <DevicePanel bind:connected />
  </aside>

  <main>
    {#if view === "library"}
      <Library />
    {:else if view === "copy"}
      <CopyScreen {connected} />
    {:else if view === "print"}
      <PrintScreen {connected} />
    {:else}
      <div class="with-pane">
        <div class="col">
          {#if session}
            <ScanScreen bind:session {connected} onexport={() => (exporting = true)} onclose={close} />
          {:else}
            <Start onopen={(s) => (session = s)} />
          {/if}
        </div>
        <aside class="pane">
          <ScanSettings {connected} />
        </aside>
      </div>
    {/if}
  </main>
</div>

{#if help}
  <Shortcuts onclose={() => (help = false)} />
{/if}

{#if exporting && session}
  <ExportDialog
    pages={session.pages.length}
    onclose={() => (exporting = false)}
    onended={() => { exporting = false; session = null; }}
  />
{/if}

<style>
  .shell { display: grid; grid-template-columns: 220px 1fr; height: 100%; }
  .side { background: var(--side); border-right: 1px solid var(--line); padding: 10px 8px 12px; display: flex; flex-direction: column; gap: 14px; min-height: 0; overflow: auto; }
  .brand { font-weight: 600; padding: 4px 8px; height: 28px; display: flex; align-items: center; }
  nav { display: grid; gap: 1px; }
  nav button { justify-content: flex-start; border: none; background: none; height: 28px; padding: 0 8px; color: var(--muted); gap: 10px; }
  nav button:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  nav button.on { background: var(--hover); color: var(--text); }
  nav button svg { transition: color 0.14s var(--ease); }
  nav button.on svg { color: var(--accent); }
  nav svg { width: 16px; height: 16px; }
  .grow { flex: 1; }
  .theme { margin: 0 4px 4px; }
  .accents { display: flex; gap: 8px; padding: 2px 8px 6px; }
  .accents button { width: 16px; height: 16px; padding: 0; border-radius: 50%; border: none; background: linear-gradient(135deg, var(--light) 50%, var(--dark) 50%); box-shadow: 0 0 0 1px var(--line-strong); cursor: pointer; transition: transform 0.1s; }
  .accents button:hover:not(:disabled) { background: linear-gradient(135deg, var(--light) 50%, var(--dark) 50%); transform: scale(1.15); }
  .accents button.on { box-shadow: 0 0 0 2px var(--side), 0 0 0 3.5px var(--text); }
  .accents button:focus-visible { outline: 2px solid var(--text); outline-offset: 2px; }
  .keys { justify-content: space-between; height: 28px; padding: 0 8px; border: none; font-size: 12px; }
  main { min-width: 0; min-height: 0; overflow: hidden; }
  .with-pane { display: flex; height: 100%; }
  .col { flex: 1; min-width: 0; overflow: auto; }
</style>
