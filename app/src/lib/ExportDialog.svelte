<script lang="ts">
  import { onMount } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, type Exported, type ExportOpts } from "./api";

  let { pages, onclose, onended }: { pages: number; onclose: () => void; onended: () => void } = $props();

  let opts = $state<ExportOpts | null>(null);
  let busy = $state(false);
  let error = $state("");
  let done = $state<Exported | null>(null);
  let langs = $state<string[] | null>(null);

  onMount(async () => {
    opts = await api.getExportOpts();
    langs = await api.ocrLanguages().catch(() => []);
    if (!langs.length) opts.searchable = false;
  });

  // Tesseract uses three-letter codes; show the language's name where the system knows it.
  const names = new Intl.DisplayNames(undefined, { type: "language" });
  const langName = (code: string) => { try { return names.of(code.split("_")[0]) ?? code; } catch { return code; } };
  const anyPdf = $derived(!!opts && (opts.color || opts.share || opts.print));

  type Flag = Exclude<keyof ExportOpts, "max_mb">;
  const outputs: [Flag, string, string][] = [
    ["color", "Colour PDF", "Full quality, as scanned"],
    ["share", "Share PDF", "Colour, 300 dpi, small enough to email"],
    ["print", "Print PDF", "Black & white with the paper whitened"],
    ["images", "Page images", "One full-quality JPEG per page"],
  ];
  const after: [Flag, string][] = [
    ["open_pdf", "Open the PDF when done"],
    ["open_folder", "Show the folder when done"],
    ["cleanup", "Delete working scans afterwards (ends the session)"],
  ];

  const anyOutput = $derived(!!opts && (opts.color || opts.share || opts.print || opts.images));

  async function run() {
    busy = true;
    error = "";
    try {
      done = await api.export($state.snapshot(opts!));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function finish() {
    if (done?.closed) onended();
    else onclose();
  }

  const base = (f: string) => f.split("/").pop();
  function onkey(e: KeyboardEvent) {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onclose();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop">
  <div class="card dialog" role="dialog" aria-label="Export">
    {#if done}
      <h2>Saved {done.files.length} {done.files.length === 1 ? "file" : "files"}</h2>
      <ul class="files">
        {#each done.files as f (f)}
          <li><button class="ghost" onclick={() => revealItemInDir(f)}>{base(f)}</button></li>
        {/each}
      </ul>
      {#if done.closed}<p class="muted">Working scans deleted; the session has ended.</p>{/if}
      <footer>
        <span class="grow"></span>
        <button class="primary" onclick={finish}>{done.closed ? "Back to start" : "Done"}</button>
      </footer>
    {:else if opts}
      <h2>Export {pages} {pages === 1 ? "page" : "pages"}</h2>
      <fieldset>
        <legend><h3>Save</h3></legend>
        {#each outputs as [k, label, hint] (k)}
          <label class="opt">
            <input type="checkbox" bind:checked={opts[k]} />
            <span><strong>{label}</strong> <span class="muted">— {hint}</span></span>
          </label>
        {/each}
      </fieldset>
      <fieldset>
        <legend><h3>Then</h3></legend>
        {#each after as [k, label] (k)}
          <label class="opt" class:danger={k === "cleanup" && opts.cleanup}>
            <input type="checkbox" bind:checked={opts[k]} />
            <span>{label}</span>
          </label>
        {/each}
      </fieldset>
      <fieldset>
        <legend><h3>Text</h3></legend>
        <label class="opt" class:off={!langs?.length || !anyPdf}>
          <input type="checkbox" bind:checked={opts.searchable} disabled={!langs?.length || !anyPdf} />
          <span><strong>Searchable PDFs</strong> <span class="muted">—
            {#if langs === null}checking for text recognition…
            {:else if !langs.length}needs Tesseract (<code>brew install tesseract</code>)
            {:else}recognise {langs.map(langName).join(" and ")} text so it can be found and copied{/if}
          </span></span>
        </label>
      </fieldset>
      <fieldset>
        <legend><h3>Size limit</h3></legend>
        <label class="opt">
          <input type="checkbox" checked={opts.max_mb !== null} onchange={(e) => (opts!.max_mb = e.currentTarget.checked ? 1 : null)} />
          <span>Fit each PDF under</span>
          <input class="mb" type="number" min="0.05" step="0.1" disabled={opts.max_mb === null}
            value={opts.max_mb ?? 1} oninput={(e) => (opts!.max_mb = Number(e.currentTarget.value) || null)} />
          <span>MB <span class="muted">— for upload limits</span></span>
        </label>
      </fieldset>
      {#if error}<p class="error">{error}</p>{/if}
      <footer>
        <button class="ghost" onclick={onclose} disabled={busy}>Cancel</button>
        <span class="grow"></span>
        <button class="primary" onclick={run} disabled={busy || !anyOutput}>{busy ? (opts.searchable && anyPdf ? "Recognising text…" : "Exporting…") : "Export"}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .dialog { width: min(520px, 92vw); padding: 20px 22px; display: grid; gap: 16px; }
  fieldset { border: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  legend { padding: 0; margin-bottom: 6px; }
  .opt { display: flex; gap: 10px; align-items: baseline; cursor: pointer; }
  .mb { width: 70px; height: 24px; }
  .opt.danger { color: var(--danger); }
  .opt.off { cursor: default; opacity: 0.6; }
  input[type="checkbox"] { accent-color: var(--accent); }
  .files { list-style: none; margin: 0; padding: 0; }
  .files button { height: 24px; padding: 0 6px; color: var(--accent); justify-content: flex-start; }
  footer { display: flex; gap: 8px; }
  .grow { flex: 1; }
  .error { color: var(--danger); margin: 0; }
  p { margin: 0; }
</style>
