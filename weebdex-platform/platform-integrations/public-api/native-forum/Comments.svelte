<script lang="ts">
 import {Comments} from '$lib/components/comments';
 import {useComments} from '$lib/components/comments/context.svelte';
 let {class:className}:{class?:string}=$props();
 const comments=useComments();
</script>
{#if comments?.thread}<Comments class={className}/>{:else}<p class={className} role="status">Comments are unavailable for this resource.</p>{/if}
