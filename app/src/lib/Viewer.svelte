<script lang="ts">
  import { onMount } from "svelte";
  import { api, fileImg } from "./api";
  import Modal from "./Modal.svelte";

  let { path, onclose }: { path: string; onclose: () => void } = $props();

  let pages = $state(1);
  let page = $state(0);
  let error = $state("");

  onMount(async () => {
    try {
      pages = await api.filePages(path);
    } catch (e) {
      error = String(e);
    }
  });

  function onkey(e: KeyboardEvent) {
    if (e.key === "ArrowRight" || e.key === "ArrowDown" || e.key === " ") page = Math.min(pages - 1, page + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") page = Math.max(0, page - 1);
    else return;
    e.preventDefault();
  }

  const name = $derived(path.split("/").pop());
</script>

<svelte:window onkeydown={onkey} />

<Modal label={name ?? "Preview"} {onclose} backdrop={false}>
<div class="viewer">
  <header>
    <span class="name">{name}</span>
    {#if pages > 1}
      <span class="nav">
        <button class="ghost" disabled={page === 0} onclick={() => page--}>‹</button>
        {page + 1} / {pages}
        <button class="ghost" disabled={page >= pages - 1} onclick={() => page++}>›</button>
      </span>
    {/if}
    <button class="ghost" onclick={onclose}>Close</button>
  </header>
  <div class="stage">
    {#if error}
      <p class="error">{error}</p>
    {:else}
      {#key page}
        <img src={fileImg(path, page)} alt="Page {page + 1}" onerror={() => (error = "This page has no scanned image to preview.")} />
      {/key}
    {/if}
  </div>
</div>
</Modal>

<style>
  .viewer { position: fixed; inset: 0; z-index: 20; background: rgb(12 12 12 / 92%); display: grid; grid-template-rows: auto 1fr; color: #eee; }
  header { display: flex; align-items: center; gap: 16px; padding: 10px 16px; }
  .name { flex: 1; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nav { display: flex; align-items: center; gap: 6px; font-variant-numeric: tabular-nums; }
  header button { color: #eee; }
  .stage { display: grid; place-items: center; padding: 0 24px 24px; min-height: 0; }
  img { max-width: 100%; max-height: calc(100vh - 80px); object-fit: contain; background: #fff; box-shadow: 0 8px 40px rgb(0 0 0 / 50%); }
  .error { color: #f0a59c; }
</style>
