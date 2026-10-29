<script lang="ts">
 import {Comments} from '$lib/components/comments';
 import {getUser} from '$lib/Context';
 import {useComments} from '$lib/components/comments/context.svelte';
 const user=getUser(),comments=useComments();
</script>
{#if comments?.thread && $user.public_account_follow_available}
 <Comments />
{:else}<p role="status">Comments are unavailable for this resource.</p>{/if}
