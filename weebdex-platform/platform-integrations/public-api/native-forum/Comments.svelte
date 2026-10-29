<script lang="ts">
 import {Comments} from '$lib/components/comments';
 import {useComments} from '$lib/components/comments/context.svelte';
 import type {NativeDiscussionTarget} from './context';
 // Public and email-session adapters address this same stored native thread.
 // Reading needs no identity choice; ordinary site authentication owns posting.
 const {class:className}:{class?:string;target?:NativeDiscussionTarget;resolveThread?:boolean}=$props();
 const comments=useComments();
</script>
{#if comments?.thread}<Comments class={className} />{:else}<p class={className} role="status">Comments are unavailable for this resource.</p>{/if}
