<script lang="ts">
  let { onclose }: { onclose: () => void } = $props();

  const groups: [string, [string, string][]][] = [
    ["Anywhere", [["⌘1 – ⌘4", "Scan, Copy, Print, Saved scans"], ["⌘/", "Show these shortcuts"], ["Esc", "Close a dialog"]]],
    ["Scan", [
      ["⌘↩", "Scan page"], ["⌘L", "Long page"], ["⌘E", "Export"],
      ["← →", "Select page"], ["Space", "Preview page"], ["R / ⇧R", "Rotate right / left"], ["⌫", "Delete page"],
    ]],
    ["Copy", [["⌘↩", "Scan / print the next step"]]],
    ["Print", [["⌘O", "Add files"], ["⌘P or ⌘↩", "Print"]]],
  ];

  function onkey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    e.preventDefault();
    onclose();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="backdrop" role="presentation" onclick={onclose}>
  <div class="card dialog" role="dialog" aria-label="Keyboard shortcuts" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
    <h2>Keyboard shortcuts</h2>
    {#each groups as [title, keys] (title)}
      <section>
        <div class="group-title">{title}</div>
        <dl>
          {#each keys as [k, what] (k)}
            <dt><kbd>{k}</kbd></dt>
            <dd>{what}</dd>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
</div>

<style>
  .dialog { width: min(420px, 92vw); max-height: 88vh; overflow-y: auto; padding: 20px 22px; display: grid; gap: 14px; }
  h2 { margin: 0; }
  dl { display: grid; grid-template-columns: 110px 1fr; gap: 6px 12px; margin: 6px 0 0; }
  dt, dd { margin: 0; }
  kbd { font: inherit; font-size: 12px; color: var(--muted); }
</style>
