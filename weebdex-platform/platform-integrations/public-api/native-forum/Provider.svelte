<script lang="ts">
 import {Provider} from '$lib/components/comments';
 import type {Snippet} from 'svelte';
 import UnavailableContext from './UnavailableContext.svelte';
 let {thread,children}:{thread?:Thread|null;children:Snippet}=$props();
</script>
{#if thread}<Provider {thread}>{@render children()}</Provider>{:else}<UnavailableContext>{@render children()}</UnavailableContext>{/if}
