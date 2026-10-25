<script lang="ts">import {getUser} from "$lib/Context";import PublisherLists from "$publicApi/PublisherLists.svelte";const user=getUser();</script>
{#if $user.public_lists_available}<PublisherLists ownerId={$user.id}/>{:else}<p class="text-muted-foreground">This profile has no published-list catalogue.</p>{/if}
