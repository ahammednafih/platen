<script lang="ts">
  import { onDestroy, type Snippet } from "svelte";
  import { Dialog, DialogBackdrop, DialogContent } from "@ui-sdk/svelte/dialog";

  interface Props {
    label: string;
    onclose: () => void;
    dismissible?: boolean;
    closeOnOutsidePress?: boolean;
    backdrop?: boolean;
    children: Snippet;
  }

  let { label, onclose, dismissible = true, closeOnOutsidePress = false, backdrop = true, children }: Props = $props();

  // The parent closes this by unmounting it, which skips the dialog's own focus return.
  const opener = document.activeElement as HTMLElement | null;
  onDestroy(() => queueMicrotask(() => { if (opener?.isConnected) opener.focus(); }));
</script>

<Dialog defaultOpen {dismissible} {closeOnOutsidePress} onOpenChange={(open, details) => { if (!open) { details.cancel(); onclose(); } }}>
  {#if backdrop}<DialogBackdrop />{/if}
  <DialogContent aria-label={label}>
    {@render children()}
  </DialogContent>
</Dialog>
