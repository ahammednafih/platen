<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fileImg, size, type Saved } from "./api";

  let { onprint }: { onprint: (path: string) => void } = $props();
  import ShrinkDialog from "./ShrinkDialog.svelte";
  import Viewer from "./Viewer.svelte";

  let items = $state<Saved[]>([]);
  let filter = $state("");
  let viewing = $state<string | null>(null);
  let shrinking = $state<string | null>(null);
  // The card whose ⋯ menu is open, and the one being renamed.
  let menu = $state<string | null>(null);
  let renaming = $state<string | null>(null);
  let draft = $state("");
  let error = $state("");

  async function share(path: string, e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    await api.shareFile(path, { x: r.left, y: r.top, w: r.width, h: r.height }).catch((err) => (error = String(err)));
  }

  async function trash(path: string) {
    menu = null;
    try {
      await api.trashFile(path);
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  function startRename(f: Saved) {
    menu = null;
    renaming = f.path;
    draft = f.name.replace(/\.[^.]+$/, "");
  }

  async function commitRename() {
    const path = renaming;
    renaming = null;
    if (!path || !draft.trim()) return;
    try {
      await api.renameFile(path, draft);
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  function renameKey(e: KeyboardEvent) {
    if (e.key === "Enter") commitRename();
    else if (e.key === "Escape") renaming = null;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  function act(f: () => unknown) {
    menu = null;
    f();
  }

  async function load() {
    items = await api.library();
  }
  onMount(load);

  async function pickPdf() {
    const f = await open({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (typeof f === "string") shrinking = f;
  }

  const shown = $derived(
    items.filter((i) => !filter || `${i.session} ${i.name}`.toLowerCase().includes(filter.toLowerCase())),
  );
  const groups = $derived.by(() => {
    const m = new Map<string, Saved[]>();
    for (const i of shown) m.set(i.session, [...(m.get(i.session) ?? []), i]);
    return [...m];
  });
  const folder = (p: string) => p.slice(0, p.lastIndexOf("/")).replace(/^\/Users\/[^/]+/, "~");
  const when = (secs: number) =>
    new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  const isPdf = (p: string) => p.toLowerCase().endsWith(".pdf");
</script>

<div class="screen">
  <header class="topbar">
    <h1>Saved scans</h1>
    <span class="grow"></span>
    <input class="search" type="text" placeholder="Search" bind:value={filter} />
    <button onclick={pickPdf}>Shrink a PDF…</button>
  </header>
  <div class="library">

  {#if error}<p class="error small">{error}</p>{/if}
  {#if !items.length}
    <p class="muted">Nothing exported yet. Exported PDFs and images show up here.</p>
  {/if}

  {#each groups as [session, files] (session)}
    <section>
      <div class="group">
        <h2>{session}</h2>
        <button class="ghost link small" onclick={() => revealItemInDir(files[0].path)}>{folder(files[0].path)}</button>
      </div>
      <ul>
        {#each files as f (f.path)}
          <li class="card">
            <button class="thumb" onclick={() => (viewing = f.path)} title="Preview">
              <span class="spinner" aria-hidden="true"></span>
              <img src={fileImg(f.path, 0, 240)} alt="" loading="lazy" onload={(e) => e.currentTarget.classList.add("loaded")} />
            </button>
            <div class="info">
              {#if renaming === f.path}
                <!-- svelte-ignore a11y_autofocus -->
                <input class="rename" aria-label="File name" bind:value={draft} autofocus onfocus={(e) => e.currentTarget.select()} onkeydown={renameKey} onblur={commitRename} />
              {:else}
                <div class="name" title={f.name}>{f.name}</div>
              {/if}
              <div class="muted small">{size(f.bytes)}, {when(f.modified)}</div>
              <div class="tools">
                <button class="ghost" onclick={() => (viewing = f.path)}>Preview</button>
                <button class="ghost" onclick={() => onprint(f.path)} title="Open in Print">Print</button>
                <button class="ghost" onclick={(e) => share(f.path, e)} title="AirDrop, Mail, Messages…">Share</button>
                <div class="more">
                  <button class="ghost dots" class:open={menu === f.path} aria-label="More" aria-expanded={menu === f.path} onclick={() => (menu = menu === f.path ? null : f.path)}>
                    <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><circle cx="3.5" cy="8" r="1.3" /><circle cx="8" cy="8" r="1.3" /><circle cx="12.5" cy="8" r="1.3" /></svg>
                  </button>
                  {#if menu === f.path}
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="catch" onclick={() => (menu = null)} onkeydown={() => {}}></div>
                    <div class="menu card" role="menu">
                      <button role="menuitem" onclick={() => act(() => openPath(f.path))}>Open</button>
                      <button role="menuitem" onclick={() => act(() => revealItemInDir(f.path))}>Show in Finder</button>
                      {#if isPdf(f.path)}<button role="menuitem" onclick={() => act(() => (shrinking = f.path))}>Shrink…</button>{/if}
                      <button role="menuitem" onclick={() => startRename(f)}>Rename</button>
                      <hr />
                      <button role="menuitem" class="danger" onclick={() => trash(f.path)}>Move to Trash</button>
                    </div>
                  {/if}
                </div>
              </div>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
  </div>
</div>

{#if viewing}
  <Viewer path={viewing} onclose={() => (viewing = null)} />
{/if}
{#if shrinking}
  <ShrinkDialog path={shrinking} onclose={() => (shrinking = null)} ondone={load} />
{/if}

<style>
  .library { padding: 16px 20px 40px; display: grid; gap: 22px; align-content: start; overflow: auto; }
  .search { width: 200px; }
  .group { display: flex; align-items: baseline; gap: 10px; margin-bottom: 8px; }
  .small { font-size: 12px; }
  .link { color: var(--accent); padding: 0; height: auto; border: none; background: none; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 10px; }
  li { display: grid; grid-template-columns: 56px 1fr; gap: 12px; padding: 10px; }
  .thumb { height: auto; padding: 0; aspect-ratio: 210 / 297; background: var(--paper); border: none; box-shadow: 0 0 0 1px var(--line-strong); border-radius: 2px; overflow: hidden; }
  .thumb { position: relative; display: grid; place-items: center; }
  .thumb > * { grid-area: 1 / 1; }
  .thumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; opacity: 0; transition: opacity 0.2s var(--ease); }
  .thumb img:global(.loaded) { opacity: 1; }
  .thumb:has(img:global(.loaded)) .spinner { display: none; }
  .info { min-width: 0; display: grid; align-content: start; gap: 2px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools { display: flex; flex-wrap: wrap; margin: 4px -6px 0; }
  .tools button { height: 22px; padding: 0 6px; font-size: 12px; }
  .tools { align-items: center; }
  .more { position: relative; }
  .dots { width: 24px; padding: 0; }
  .dots.open { background: var(--hover); color: var(--text); }
  .catch { position: fixed; inset: 0; z-index: 19; }
  .menu {
    position: absolute; top: calc(100% + 4px); left: 0; z-index: 20; min-width: 168px; padding: 4px; display: grid;
    background: var(--raised); border: none; border-radius: 8px; box-shadow: var(--shadow-pop); animation: pop-in 0.14s var(--ease);
  }
  .menu button { justify-content: flex-start; height: 26px; font-size: 13px; padding: 0 10px; border: none; background: none; border-radius: 5px; }
  .menu button:hover:not(:disabled) { background: var(--accent); color: var(--accent-text); }
  .menu button.danger { color: var(--danger); }
  .menu button.danger:hover:not(:disabled) { background: var(--danger); color: #fff; }
  .menu hr { border: none; border-top: 1px solid var(--line); margin: 4px 6px; }
  .rename { height: 24px; font-weight: 600; }
  .error { color: var(--danger); }
</style>
