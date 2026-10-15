<script lang="ts">
 import {Votes} from '$lib/components/comments';
 import {useComments} from '$lib/components/comments/context.svelte';
 const comments=useComments();
</script>
{#if comments?.thread}<Votes/>{/if}
