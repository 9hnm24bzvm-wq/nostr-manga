<script lang="ts">
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import { openLibrary, message } from './library';
  import RemoteSignerStatus from './RemoteSignerStatus.svelte';

  // The root shell owns startup and errors once, including standalone readers.
  // Management UI can be dismissed without discarding the persistent library.
  onMount(() => {
    const reportError = (event: Event) => toast.error('Library save failed', { description: String((event as CustomEvent).detail) });
    window.addEventListener('weebdex-library-error', reportError);
    void openLibrary().catch(cause => toast.error('Library unavailable', { description: message(cause) }));
    return () => window.removeEventListener('weebdex-library-error', reportError);
  });
</script>

<RemoteSignerStatus />
