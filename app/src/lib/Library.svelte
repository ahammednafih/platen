<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, fileImg, size, type Saved } from "./api";
  import ShrinkDialog from "./ShrinkDialog.svelte";
  import Viewer from "./Viewer.svelte";

  let items = $state<Saved[]>([]);
  let filter = $state("");
  let viewing = $state<string | null>(null);
  let shrinking = $state<string | null>(null);

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

<div class="library">
  <header>
    <h1>Saved scans</h1>
    <input type="text" placeholder="Search" bind:value={filter} />
    <button onclick={pickPdf}>Shrink a PDF…</button>
  </header>

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
              <img src={fileImg(f.path, 0, 240)} alt="" loading="lazy" />
            </button>
            <div class="info">
              <div class="name" title={f.name}>{f.name}</div>
              <div class="muted small">{size(f.bytes)} · {when(f.modified)}</div>
              <div class="tools">
                <button class="ghost" onclick={() => (viewing = f.path)}>Preview</button>
                <button class="ghost" onclick={() => openPath(f.path)}>Open</button>
                <button class="ghost" onclick={() => revealItemInDir(f.path)}>Show in Finder</button>
                {#if isPdf(f.path)}
                  <button class="ghost" onclick={() => (shrinking = f.path)}>Shrink…</button>
                {/if}
              </div>
            </div>
          </li>
        {/each}
      </ul>
    </section>
  {/each}
</div>

{#if viewing}
  <Viewer path={viewing} onclose={() => (viewing = null)} />
{/if}
{#if shrinking}
  <ShrinkDialog path={shrinking} onclose={() => (shrinking = null)} ondone={load} />
{/if}

<style>
  .library { padding: 20px 28px 40px; display: grid; gap: 22px; align-content: start; }
  header { display: flex; gap: 12px; align-items: center; }
  header h1 { flex: 1; }
  header input { width: 220px; }
  .group { display: flex; align-items: baseline; gap: 10px; margin-bottom: 8px; }
  .small { font-size: 12px; }
  .link { color: var(--accent); padding: 0; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 10px; }
  li { display: grid; grid-template-columns: 72px 1fr; gap: 12px; padding: 10px; }
  .thumb { padding: 0; aspect-ratio: 210 / 297; background: #fff; border: 1px solid var(--line); border-radius: 3px; overflow: hidden; }
  .thumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; }
  .info { min-width: 0; display: grid; align-content: start; gap: 2px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tools { display: flex; flex-wrap: wrap; margin: 4px -6px 0; }
  .tools button { padding: 2px 6px; font-size: 12px; color: var(--accent); }
</style>
