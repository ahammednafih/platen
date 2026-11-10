<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Draft, type SessionView } from "./api";

  let { onopen }: { onopen: (s: SessionView) => void } = $props();

  let drafts = $state<Draft[]>([]);
  let name = $state("");
  let error = $state("");

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
          <li>
            <button class="draft" onclick={() => open(d.name)}>
              <span class="dname">{d.name}</span>
              <span class="muted">{d.pages} {d.pages === 1 ? "page" : "pages"} · {when(d.modified)}</span>
              <span class="resume">Resume →</span>
            </button>
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
  .dname { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resume { color: var(--accent); font-weight: 600; }
</style>
