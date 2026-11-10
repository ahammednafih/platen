<script lang="ts">
  import { img, mm, type Page } from "./api";

  let { pages, index = $bindable(), onclose }: { pages: Page[]; index: number; onclose: () => void } = $props();

  const page = $derived(pages[index]);

  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape" || e.key === " ") onclose();
    else if (e.key === "ArrowRight" || e.key === "ArrowDown") index = Math.min(pages.length - 1, index + 1);
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") index = Math.max(0, index - 1);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onkey} />

{#if page}
  <div class="backdrop" role="presentation" onclick={onclose}>
    <img src={img("pages", page.file, 1800)} alt="Page {index + 1}" />
    <div class="caption">Page {index + 1} of {pages.length} · {mm(page).join(" × ")} mm · {page.dpi} dpi</div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed; inset: 0; z-index: 20; background: rgb(10 10 10 / 88%);
    display: grid; grid-template-rows: 1fr auto; place-items: center; padding: 24px; gap: 12px;
  }
  img { max-width: 100%; max-height: calc(100vh - 90px); object-fit: contain; box-shadow: 0 8px 40px rgb(0 0 0 / 50%); background: #fff; }
  .caption { color: #ddd; font-size: 13px; }
</style>
