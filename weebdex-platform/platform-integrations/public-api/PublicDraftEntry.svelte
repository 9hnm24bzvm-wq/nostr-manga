<script lang="ts">
 import {reviewTimeline} from './draft-feedback';
 import { onDestroy } from 'svelte';
 import { Constants, Locale } from '$lib';
 import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
 import { buttonVariants } from '$lib/components/ui/button';
 import Button from '$lib/components/ui/button/button.svelte';
 import { Label } from '$lib/components/ui/label';
 import { ExternalLinkIcon, Link, Pencil, SendIcon, TrashIcon } from '@lucide/svelte';
 import { draftGrants, openSessionDraftCommand } from './session-draft-client';
 import type { MangaView } from './types';
 import type { SignedTitleController, TitleAccount } from './title-client';
 let {nativeAccount,manga,account,assertCurrent,onChanged}:{nativeAccount:string;manga:MangaView;account:TitleAccount;assertCurrent:()=>void;onChanged:(value:MangaView|undefined)=>void}=$props();
 let disposed=false,busy=$state(false),pending=$state(false),removalQueued=$state(false),message=$state(''),controller:SignedTitleController|undefined;
 const feedback=$derived(reviewTimeline(manga.public_api.resource.attributes.reviewTimeline).events.filter(e=>e.action==='rejected').at(-1));
 const grants=$derived(draftGrants(account,manga));
 const creator=$derived(manga.public_api.resource.relationships.find(row=>row.type==='creator'&&typeof row.attributes?.username==='string'));
 const submitter=$derived(creator?{id:creator.id,name:String(creator.attributes!.username)}:undefined);
 onDestroy(()=>{disposed=true;controller?.dispose();});
 async function run(remove=false){
  if(disposed||busy||pending||removalQueued)return;
  const target=manga.id,version=manga.version,state=manga.state,href=window.location.href,mode=grants.hide?'approve-hide':'request-review';
  if(remove?!grants.remove:!grants.submit||!['draft','rejected'].includes(state))return;
  const check=()=>{assertCurrent();if(disposed||manga.id!==target||manga.version!==version||manga.state!==state||window.location.href!==href)throw Error('The draft card or route changed. No stale command was continued.');};
  busy=true;message='';
  try{controller=await openSessionDraftCommand(nativeAccount,manga,account,check);check();pending=controller.pending();
   if(remove){const receipt=await controller.disposition('remove',mode);check();if(receipt.status===204){message='Draft soft-hidden on this instance. Original events and bytes are unchanged.';onChanged(undefined);}else if(receipt.status===202){removalQueued=true;message='Removal review queued: '+receipt.requestId+'. The draft remains here until approved.';}else throw Error('Draft removal was not confirmed.');}
   else{const receipt=await controller.mutate('submit');check();if(receipt.status!==201||!receipt.body)throw Error('Draft submission refused (HTTP '+receipt.status+'). Reload before trying again.');message='Draft submitted for review.';onChanged(receipt.body);}
  }catch(error){if(!disposed)message=error instanceof Error?error.message:'The draft action was not confirmed.';}
  finally{if(!disposed){busy=false;pending=controller?.pending()??pending;}controller?.dispose();controller=undefined;}
 }
 const onCommit=()=>run(false),onDelete=()=>run(true);
</script>

<article
	class="bg-input/30 relative flex flex-col space-y-5 overflow-hidden rounded-md border-l-2 p-2 pl-3"
	class:border-l-slate-500={manga.state === Constants.State.Draft}
	class:border-l-red-500={manga.state === Constants.State.Rejected}
	class:border-l-orange-500={manga.state === Constants.State.Submitted}
	class:border-l-green-500={manga.state === Constants.State.Published}
>
	<div class="flex flex-auto">
		<div class="w-0 flex-auto space-y-2">
			<strong class="text-muted-foreground block font-medium">#{manga.id}</strong>
			<div class="flex flex-wrap items-center gap-x-4 gap-y-2">
				{#if submitter}
					<div class="space-y-1">
						<Label>Created by</Label>
						<a
							class="hover:text-accent-foreground flex items-center gap-1"
							href="/user/{submitter.id}"
							title={submitter.name}
						>
							<Link class="size-3.5 shrink-0" />
							{submitter.name}
						</a>
					</div>
				{/if}
				<div class="space-y-1">
					<Label>Title</Label>
					<div class="gap-2">
						<img
							class="inline size-4 shrink-0"
							src="/images/flags/{Locale.to3166(manga.language)}.svg"
							alt={Locale.toLanguage(manga.language)}
						/>
						<h3 class="inline font-medium">{manga.title}</h3>
					</div>
				</div>
			</div>
		</div>
		<div class="shrink-0">
			{#if (manga.state === Constants.State.Draft || manga.state === Constants.State.Rejected) && grants.submit}
				<ConfirmDialog
					title="Submit Draft"
					description="Are you sure you want to submit this draft?"
					confirmText="Submit"
					variant="default"
					onConfirm={e => e.exec(onCommit)}
					disabled={busy || pending || removalQueued}
				>
					{#snippet child({ props })}
						<Button variant="ghost" size="icon-sm" title="Submit" {...props}>
							<SendIcon class="size-5" />
						</Button>
					{/snippet}
				</ConfirmDialog>
			{/if}
			{#if grants.remove}
				<ConfirmDialog
					title={grants.hide ? "Hide Draft on This Instance" : "Request Draft Removal Review"}
					description={grants.hide ? "Approve hiding this draft on this instance now? Original events and image bytes remain unchanged." : "Request moderator review? The draft stays here until approved. No original deletion is requested."}
					confirmText={grants.hide ? "Hide" : "Request review"}
					variant="destructive"
					onConfirm={e => e.exec(onDelete)}
					disabled={busy || pending || removalQueued}
				>
					{#snippet child({ props })}
						<Button variant="ghost" size="icon-sm" title={grants.hide ? "Hide on this instance" : "Request removal review"} {...props}>
							<TrashIcon class="size-5" />
						</Button>
					{/snippet}
				</ConfirmDialog>
			{/if}
			{#if grants.edit}
				<a
					class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
					href={`/title/${manga.id}/edit?draft=true`}
					title="Edit"
				>
					<Pencil class="size-5" />
				</a>
			{/if}
			<a
				class={buttonVariants({ class: "cursor-pointer", variant: "ghost", size: "icon-sm" })}
				href={`/title/${manga.id}?draft=true`}
				title={manga.state === Constants.State.Published ? "View title" : "Preview draft"}
			>
				<ExternalLinkIcon class="size-5" />
			</a>
		</div>
	</div>
	<div class="flex items-center gap-2">
		<div class="w-0 flex-auto"></div>
		<div class="flex shrink-0 items-center gap-2">
			<time datetime={manga.created_at}>{manga.created_at ?? ""}</time>
			<strong
				class="font-semibold"
				class:text-slate-400={manga.state === Constants.State.Draft}
				class:text-orange-400={manga.state === Constants.State.Submitted}
				class:text-red-400={manga.state === Constants.State.Rejected}
				class:text-green-400={manga.state === Constants.State.Published}
				>[{Constants.StateText[manga.state]}]
			</strong>
		</div>
	</div>
 {#if manga.state==='rejected'}<div class="rounded border p-3"><strong>Changes requested</strong>{#if feedback}<p>{feedback.reason}</p>{:else}<p>Open the draft to review its details.</p>{/if}<a class="underline" href={`/title/${manga.id}/edit?draft=true`}>Edit and resubmit</a></div>{/if}
 {#if message}<p class="text-sm" role="status">{message}</p>{/if}
 {#if pending}<p class="text-sm" role="alert">An earlier outcome is unconfirmed. No command will be replayed, even after reloading this list.</p>{/if}
</article>
