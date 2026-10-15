<script lang="ts">
 import { setContext,type Snippet } from 'svelte';
 let {children}:{children:Snippet}=$props();
 // No thread ID, zero counters, actor or write authority is supplied.
 setContext('comments',Object.freeze({thread:undefined,stats:undefined,feed:undefined,voteState:undefined,unavailable:true}));
</script>
{@render children()}
