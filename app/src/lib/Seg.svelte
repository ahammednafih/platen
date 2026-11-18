<script lang="ts" generics="T">
  import { SegmentedControl, SegmentedControlIndicator, SegmentedControlItem } from "@ui-sdk/svelte/segmented-control";

  type Option = { value: T; label: string; title?: string; disabled?: boolean };

  let { value, options, onchange, class: cls = "", ...rest }: {
    value: T | null;
    options: Option[];
    onchange: (v: T) => void;
    class?: string;
    title?: string;
    "aria-label"?: string;
    "aria-labelledby"?: string;
  } = $props();

  const key = (v: T) => String(v);
</script>

<SegmentedControl {...rest} class="seg {cls}" value={value === null ? null : key(value)}
  onValueChange={(v) => { const o = options.find((o) => key(o.value) === v); if (o) onchange(o.value); }}>
  <SegmentedControlIndicator />
  {#each options as o (key(o.value))}
    <SegmentedControlItem value={key(o.value)} disabled={o.disabled} title={o.title}>{o.label}</SegmentedControlItem>
  {/each}
</SegmentedControl>
