<script lang="ts">
  import { getManga } from '$lib/Context';
  import * as Comments from '$lib/components/comments';
  import * as PublicComments from '../public-api/native-forum';
  import { publicUuidLibraryEnabled } from '../public-api/library-staging';
  import ProtocolIdentityPanel from './ProtocolIdentityPanel.svelte';
  const manga = getManga();
</script>
<div class="space-y-5">
  {#if publicUuidLibraryEnabled}
    <PublicComments.Comments target={{kind:'manga',targetId:manga().id,threadId:manga().relationships.thread?.id??''}} />
  {:else if manga().relationships.thread}<Comments.Comments />{/if}
  <ProtocolIdentityPanel resourceKind="manga" resourceId={manga().id} />
</div>
