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

<div class="start">
  <div class="card new">
    <h1>New scan</h1>
    <p class="muted">Pages are saved as you go, so you can close the app and come back to a draft.</p>
    <form onsubmit={(e) => { e.preventDefault(); open(name); }}>
      <input type="text" placeholder="Name (optional, e.g. passport-renewal)" bind:value={name} />
      <button class="primary">Start</button>
    </form>
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  <div class="drafts">
    <h3>Drafts</h3>
    {#if drafts.length}
      <ul>
        {#each drafts as d (d.name)}
          <li class="row">
            <button class="draft" onclick={() => open(d.name)}>
              <span class="dname">{d.name}</span>
              <span class="muted">{d.pages} {d.pages === 1 ? "page" : "pages"} · {when(d.modified)}</span>
              <span class="resume">Resume →</span>
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

<style>
  .start { max-width: 640px; margin: 48px auto; padding: 0 24px; display: grid; gap: 32px; }
  .new { padding: 24px; display: grid; gap: 12px; }
  .new p { margin: 0; }
  form { display: flex; gap: 8px; }
  .error { color: var(--danger); }
  .drafts { display: grid; gap: 10px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .draft {
    width: 100%; display: grid; grid-template-columns: 1fr auto auto; gap: 16px;
    align-items: center; text-align: left; padding: 10px 14px;
  }
  .row { display: flex; gap: 6px; align-items: center; }
  .row .draft { flex: 1; }
  .del { color: var(--muted); font-size: 12px; }
  .del:hover { color: var(--danger); }
  .confirm { display: flex; gap: 4px; align-items: center; }
  .small { font-size: 12px; padding: 4px 8px; }
  .dname { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resume { color: var(--accent); font-weight: 600; }
</style>
