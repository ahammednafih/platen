<script lang="ts">
  import { api, img, type PageResult, type TopHalf } from "./api";

  let { replace, at, ondone, oncancel }: {
    replace?: number;
    at?: number;
    ondone: (r: PageResult) => void;
    oncancel: () => void;
  } = $props();

  type Step = "top" | "scanning-top" | "flip" | "joining";
  let step = $state<Step>("top");
  let top = $state<TopHalf | null>(null);
  let error = $state("");

  async function scanTop() {
    step = "scanning-top";
    error = "";
    try {
      top = await api.scanTop();
      step = "flip";
    } catch (e) {
      error = String(e);
      step = "top";
    }
  }

  async function scanBottom() {
    step = "joining";
    error = "";
    try {
      ondone(await api.scanBottom(top!, { replace, at }));
    } catch (e) {
      error = String(e);
      step = "flip";
    }
  }

  const busy = $derived(step === "scanning-top" || step === "joining");
  const topFile = $derived(top ? top.path.split("/").pop()! : "");
</script>

<div class="backdrop">
  <div class="card dialog" role="dialog" aria-label="Long page">
    <header>
      <h2>{replace === undefined ? "Long page" : `Retake page ${replace + 1} (long)`}</h2>
      <ol class="steps">
        <li class:on={step === "top" || step === "scanning-top"}>1. Top half</li>
        <li class:on={step === "flip" || step === "joining"}>2. Bottom half</li>
      </ol>
    </header>

    <div class="body">
      <div class="diagram" aria-hidden="true">
        <div class="glass"></div>
        <div class="sheet" class:flipped={step === "flip" || step === "joining"}></div>
      </div>
      <div class="text">
        {#if step === "top" || step === "scanning-top"}
          <p>Put the <strong>top</strong> of the page face down on the glass, top edge against the far edge.</p>
          <p class="muted">The rest of the sheet can hang off the bottom.</p>
        {:else}
          <p>Turn the page around so the <strong>bottom</strong> is on the glass, bottom edge against the far edge.</p>
          <p class="muted">Leave a few centimetres of overlap with the top scan so the halves can be matched.</p>
          {#if top}
            <figure>
              <img src={img("raw", topFile, 280)} alt="Top half" />
              <figcaption class="muted">Top half. Check it before turning the page.</figcaption>
            </figure>
          {/if}
        {/if}
        {#if error}<p class="error">{error}</p>{/if}
      </div>
    </div>

    <footer>
      <button class="ghost" onclick={oncancel} disabled={busy}>Cancel</button>
      <span class="grow"></span>
      {#if step === "flip"}
        <button onclick={scanTop}>Rescan top</button>
      {/if}
      {#if step === "top" || step === "scanning-top"}
        <button class="primary" onclick={scanTop} disabled={busy}>{busy ? "Scanning top…" : "Scan top half"}</button>
      {:else}
        <button class="primary" onclick={scanBottom} disabled={busy}>{busy ? "Scanning and joining…" : "Scan bottom half"}</button>
      {/if}
    </footer>
  </div>
</div>

<style>
  .dialog { width: min(640px, 92vw); padding: 20px 22px; display: grid; gap: 18px; }
  header { display: flex; justify-content: space-between; align-items: baseline; }
  .steps { display: flex; gap: 14px; list-style: none; margin: 0; padding: 0; font-size: 12px; color: var(--muted); }
  .steps .on { color: var(--accent); font-weight: 600; }
  .body { display: grid; grid-template-columns: 120px 1fr; gap: 22px; }
  .text p { margin: 0 0 8px; }
  .diagram { position: relative; height: 200px; }
  .glass { position: absolute; left: 10px; right: 10px; top: 0; height: 110px; border: 2px dashed var(--muted); border-radius: 6px; }
  .sheet {
    position: absolute; left: 22px; right: 22px; top: 6px; height: 186px; border-radius: 3px;
    background: linear-gradient(var(--accent) 0 30%, var(--sunken) 30%); border: 1px solid var(--line);
    transition: transform 0.4s;
  }
  .sheet.flipped { background: linear-gradient(var(--warn) 0 30%, var(--sunken) 30%); }
  figure { margin: 10px 0 0; }
  figure img { max-height: 180px; max-width: 100%; border: 1px solid var(--line); border-radius: 4px; display: block; }
  figcaption { font-size: 12px; margin-top: 4px; }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
  .error { color: var(--danger); }
</style>
