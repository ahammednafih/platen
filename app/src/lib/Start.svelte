<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Draft, type SessionView } from "./api";

  let { onopen }: { onopen: (s: SessionView) => void } = $props();

  let drafts = $state<Draft[]>([]);
  let name = $state("");
  let error = $state("");
  let confirming = $state<string | null>(null);

  async function remove(n: string) {
    error = "";
    try {
      drafts = await api.deleteDraft(n);
    } catch (e) {
      error = String(e);
    }
    confirming = null;
  }

  async function open(n?: string) {
    error = "";
    try {
      onopen(await api.openSession(n));
    } catch (e) {
      error = String(e);
    }
  }

  onMount(async () => {
    drafts = await api.drafts();
  });

  const when = (secs: number) =>
    new Date(secs * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
</script>

<div class="screen">
<header class="topbar"><h1>Scan</h1></header>
<div class="start">
  <div class="new">
    <span class="tile" aria-hidden="true"><svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" stroke-linecap="round"><path d="M3.5 2.5h6l3 3v8h-9z M9.5 2.5v3h3 M1.5 9h13" /></svg></span>
    <h1>New scan</h1>
    <p class="muted">Pages are saved as you go, so you can close the app and come back to a draft.</p>
    <form onsubmit={(e) => { e.preventDefault(); open(name); }}>
      <input type="text" placeholder="Name (optional, e.g. passport-renewal)" bind:value={name} />
      <button class="primary">Start</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  <div class="drafts">
    <h3>Drafts{#if drafts.length} <span class="count">{drafts.length}</span>{/if}</h3>
    {#if drafts.length}
      <ul>
        {#each drafts as d (d.name)}
          <li class="row">
            <button class="draft" onclick={() => open(d.name)}>
              <span class="dname">{d.name}</span>
              <span class="muted meta">{d.pages} {d.pages === 1 ? "page" : "pages"}, {when(d.modified)}</span>
              <span class="resume">Resume</span>
            </button>
            {#if confirming === d.name}
              <span class="confirm">
                <span class="muted small">Delete scans?</span>
                <button class="danger small" onclick={() => remove(d.name)}>Delete</button>
                <button class="ghost small" onclick={() => (confirming = null)}>Keep</button>
              </span>
            {:else}
              <button class="ghost del" title="Delete draft (exported files are kept)" onclick={() => (confirming = d.name)}>Delete</button>
            {/if}
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted">No drafts. Sessions you leave without exporting show up here.</p>
    {/if}
  </div>
</div>
</div>

<style>
  .start { max-width: 600px; width: 100%; margin: 56px auto; padding: 0 24px; display: grid; gap: 36px; }
  .new { display: grid; gap: 10px; padding: 24px; border: 1px solid var(--line); border-radius: 12px; background: var(--panel); box-shadow: var(--shadow); }
  .tile { width: 36px; height: 36px; border-radius: 9px; display: grid; place-items: center; background: var(--accent-soft); color: var(--accent); margin-bottom: 4px; }
  .tile svg { width: 18px; height: 18px; }
  .count { font-size: 11px; color: var(--muted); background: var(--sunken); border: 1px solid var(--line); border-radius: 999px; padding: 0 6px; margin-left: 4px; font-variant-numeric: tabular-nums; }
  .new h1 { font-size: 20px; }
  form { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px; }
  form input { flex: 1 1 180px; }
  .drafts { display: grid; gap: 8px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; border-top: 1px solid var(--line); }
  .row { display: flex; gap: 6px; align-items: center; border-bottom: 1px solid var(--line); }
  .draft {
    flex: 1; height: 40px; display: grid; grid-template-columns: 1fr auto auto; gap: 16px;
    align-items: center; text-align: left; padding: 0 8px; border: none; background: none; border-radius: 0;
  }
  .draft:hover .resume { opacity: 1; }
  .del { font-size: 12px; height: 24px; }
  .del:hover:not(:disabled) { color: var(--danger); }
  .confirm { display: flex; gap: 4px; align-items: center; }
  .small { font-size: 12px; }
  .meta { text-align: right; font-variant-numeric: tabular-nums; }
  .dname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resume { color: var(--accent); opacity: 0; transition: opacity 0.12s; }
</style>
