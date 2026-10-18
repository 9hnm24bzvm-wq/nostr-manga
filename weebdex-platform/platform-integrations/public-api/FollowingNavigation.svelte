<script lang="ts">
 import {buttonVariants} from '$lib/components/ui/button';
 import {cn} from '$lib/Utils';
 import type{AccountFollowKind} from './account-follow-client';
 const {active}:{active:AccountFollowKind|'list'}=$props();
 const tabs=[{kind:'manga',name:'Titles',path:'/following/manga'},{kind:'group',name:'Groups',path:'/following/group'},{kind:'user',name:'Users',path:'/following/user'},{kind:'list',name:'Lists',path:'/following/lists'}];
</script>
<div class="border-b-2"><nav class="text-muted-foreground/80 -mb-0.5 flex flex-nowrap overflow-auto whitespace-nowrap" aria-label="Account following">
 {#each tabs as tab}<a href={tab.path} aria-current={active===tab.kind?'page':undefined} class={buttonVariants({class:cn('border-b-2 border-b-transparent !bg-transparent',{'border-accent !text-accent-foreground':active===tab.kind}),variant:'ghost'})}>{tab.name}</a>{/each}
</nav></div>
