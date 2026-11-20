<script lang="ts">
  import Modal from "./Modal.svelte";

  let { count, index = $bindable(), caption, render, onclose }: {
    count: number;
    index: number;
    caption: (i: number) => string;
    render: (i: number) => Promise<string>;
    onclose: () => void;
  } = $props();

  let urls = $state<Record<number, string>>({});
  let errors = $state<Record<number, string>>({});
  const url = $derived(urls[index] ?? "");
  const error = $derived(errors[index] ?? "");

  $effect(() => {
    const draw = render;
    let stale = false;
    const pending = new Set<number>();
    const get = (i: number) => {
      if (i < 0 || i >= count || i in urls || pending.has(i)) return;
      pending.add(i);
      draw(i).then(
        (u) => { if (stale) URL.revokeObjectURL(u); else urls[i] = u; },
        (e) => { if (!stale) errors[i] = String(e); },
      );
    };
    $effect(() => {
      get(index);
      get(index + 1);
      get(index - 1);
    });
    return () => {
      stale = true;
      Object.values(urls).forEach((u) => URL.revokeObjectURL(u));
      urls = {};
      errors = {};
    };
  });

  function onkey(e: KeyboardEvent) {
    if (e.metaKey) return;
    if (e.key === " ") onclose();
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") index = Math.min(count - 1, index + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") index = Math.max(0, index - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

<Modal label={caption(index)} {onclose} backdrop={false}>
  <div class="backdrop" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
    <div class="stage" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
      {#if error}
        <p class="err" role="alert">{error}</p>
      {:else if url}
        <img src={url} alt={caption(index)} />
      {:else}
        <span class="spinner"></span>
      {/if}
    </div>
    <div class="bar">
      <button class="ghost small" onclick={() => index--} disabled={index === 0} aria-label="Previous">‹</button>
      <span class="caption">{caption(index)} of {count}</span>
      <button class="ghost small" onclick={() => index++} disabled={index >= count - 1} aria-label="Next">›</button>
      <button class="ghost small" onclick={onclose}>Close</button>
    </div>
  </div>
</Modal>

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 20; background: rgb(10 10 10 / 88%);
    display: grid; grid-template-rows: 1fr auto; place-items: center; padding: 24px; gap: 12px;
  }
  .stage { display: grid; place-items: center; width: 100%; height: 100%; min-height: 0; }
  img { max-width: 100%; max-height: calc(100vh - 90px); object-fit: contain; box-shadow: 0 8px 40px rgb(0 0 0 / 50%); background: #fff; }
  .bar { display: flex; align-items: center; gap: 10px; }
  .caption { color: #ddd; font-size: 0.8125rem; font-variant-numeric: tabular-nums; }
  .bar button { color: #ddd; font-size: 0.875rem; }
  .bar button:hover:not(:disabled) { background: rgb(255 255 255 / 10%); color: #fff; }
  .err { color: #f0a59c; font-size: 0.8125rem; }
</style>
