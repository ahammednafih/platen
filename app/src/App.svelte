<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
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
  let appearing = $state(false);
  // A Scan-button press waiting for the scan screen to pick it up.
  let press = $state<number | null>(null);

  // The device waits for this Mac after a press, so open a session and go to Scan right away.
  onMount(() => {
    const un = listen<{ seq: number }>("button-press", async (e) => {
      if (exporting) return;
      view = "scan";
      if (!session) session = await api.openSession();
      press = e.payload.seq;
    });
    return () => { un.then((f) => f()); };
  });

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
    if (e.key === "Escape" && appearing) return void (appearing = false);
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
    <div class="brand"><span class="mark" aria-hidden="true"></span>Scan</div>
    <nav>
      {#each nav as [v, label, key] (v)}
        <button class:on={view === v} onclick={() => (view = v)} title="{label} (⌘{key})">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"><path d={icons[v]} /></svg>
          {label}
        </button>
      {/each}
    </nav>
    <div class="grow"></div>
    <DevicePanel bind:connected />
    <div class="foot">
      <div class="pop-anchor">
        <button class="ghost small" class:open={appearing} onclick={() => (appearing = !appearing)} aria-expanded={appearing}>
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" aria-hidden="true"><circle cx="8" cy="8" r="5.5" /><path d="M8 2.5v11A5.5 5.5 0 0 0 8 2.5z" fill="currentColor" /></svg>
          Appearance
        </button>
        {#if appearing}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="catch" onclick={() => (appearing = false)} onkeydown={() => {}}></div>
          <div class="pop card" role="dialog" aria-label="Appearance">
            <div class="seg theme">
              {#each [["system", "System"], ["light", "Light"], ["dark", "Dark"]] as [v, label] (v)}
                <button class:on={theme === v} onclick={() => (theme = v as Theme)}>{label}</button>
              {/each}
            </div>
            <div class="accents" role="radiogroup" aria-label="Accent colour">
              {#each accents as [v, label, light, dark] (v)}
                <button role="radio" aria-checked={accent === v} class:on={accent === v} title={label} aria-label={label} style:--light={light} style:--dark={dark} onclick={() => (accent = v)}></button>
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <button class="ghost small" onclick={() => (help = true)} title="Keyboard shortcuts (⌘/)">Shortcuts <span class="faint">⌘/</span></button>
    </div>
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
            <ScanScreen bind:session {connected} {press} onpressed={() => (press = null)} onexport={() => (exporting = true)} onclose={close} />
          {:else}
            <Start onopen={(s) => (session = s)} />
          {/if}
        </div>
        <aside class="pane scan-pane">
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
  .brand { font-weight: 600; padding: 4px 8px; height: 28px; display: flex; align-items: center; gap: 8px; letter-spacing: -0.01em; }
  .mark { width: 18px; height: 18px; border-radius: 5px; background: var(--accent); position: relative; transition: background-color 0.25s var(--ease); }
  .mark::after { content: ""; position: absolute; left: 3px; right: 3px; top: 8px; height: 2px; border-radius: 1px; background: var(--accent-text); opacity: 0.9; }
  .foot { display: flex; justify-content: space-between; align-items: center; padding-top: 8px; border-top: 1px solid var(--line); }
  .foot button.open { background: var(--hover); color: var(--text); }
  .pop-anchor { position: relative; }
  .catch { position: fixed; inset: 0; z-index: 19; }
  .pop {
    position: absolute; bottom: calc(100% + 6px); left: 0; z-index: 20; width: 196px; padding: 10px; display: grid; gap: 10px;
    background: var(--raised); box-shadow: var(--shadow-pop); border: none; border-radius: 10px; animation: pop-in 0.16s var(--ease);
  }
  nav { display: grid; gap: 1px; }
  nav button { justify-content: flex-start; border: none; background: none; height: 28px; padding: 0 8px; color: var(--muted); gap: 10px; }
  nav button:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  nav button.on { background: var(--hover); color: var(--text); }
  nav button svg { transition: color 0.14s var(--ease); }
  nav button.on svg { color: var(--accent); }
  nav svg { width: 16px; height: 16px; }
  .grow { flex: 1; }
  .theme { margin: 0; }
  .accents { display: flex; justify-content: space-between; padding: 0 4px; }
  .accents button { width: 14px; height: 14px; padding: 0; border-radius: 50%; border: none; background: linear-gradient(135deg, var(--light) 50%, var(--dark) 50%); box-shadow: 0 0 0 1px var(--line-strong); cursor: pointer; transition: transform 0.1s; }
  .accents button:hover:not(:disabled) { background: linear-gradient(135deg, var(--light) 50%, var(--dark) 50%); transform: scale(1.15); }
  .accents button { width: 18px; height: 18px; }
  .accents button.on { box-shadow: 0 0 0 2px var(--raised), 0 0 0 3.5px var(--text); }
  .accents button:focus-visible { outline: 2px solid var(--text); outline-offset: 2px; }
  main { min-width: 0; min-height: 0; overflow: hidden; }
  .with-pane { display: flex; height: 100%; }
  .col { flex: 1; min-width: 0; overflow: auto; }
  .scan-pane { padding-top: 0; }
</style>
