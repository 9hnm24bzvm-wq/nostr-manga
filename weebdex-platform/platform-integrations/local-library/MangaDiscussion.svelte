<script lang="ts">
  import { getManga } from '$lib/Context';
  import * as Comments from '$lib/components/comments';
  import ProtocolIdentityPanel from './ProtocolIdentityPanel.svelte';
  const manga = getManga();
</script>
<div class="space-y-5">
  {#if manga().relationships.thread}<Comments.Comments />{/if}
  <ProtocolIdentityPanel resourceKind="manga" resourceId={manga().id} />
</div>
