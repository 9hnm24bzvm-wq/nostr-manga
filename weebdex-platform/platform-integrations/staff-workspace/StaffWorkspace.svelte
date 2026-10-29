<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {Session} from '$lib/stores';
 import {createStaffClient,can,type Authority,type StaffClient} from './staff-client';
 let client:StaffClient|undefined,epoch=0,closed=false;let authority=$state<Authority>(),error=$state(''),pending=$state(false);
 const admin=$derived(!!authority?.roles.includes('sysop')&&authority.permissions.some(p=>['admin.users.read','admin.roles.manage','admin.restrictions.manage','admin.sessions.revoke','admin.audit.read'].includes(p)));
 $effect(()=>{const session=$Session;untrack(()=>{epoch++;client?.dispose();client=undefined;authority=undefined;error='';pending=false;if(session?.identity?.id)void load();});});
 onDestroy(()=>{closed=true;epoch++;client?.dispose();});
 async function load(){if(pending||!$Session?.identity?.id)return;const n=epoch,session=$Session,assert=()=>{if(closed||n!==epoch||session!==$Session)throw Error('Account changed');};pending=true;error='';try{client=await createStaffClient(session.identity.id,assert);assert();authority=client.authority;}catch(cause){try{assert();error=cause instanceof Error?cause.message:'Staff access could not be checked.';}catch{}}finally{try{assert();pending=false;}catch{}}}
</script>
<svelte:head><title>Staff workspace</title></svelte:head>
<section class="space-y-5">
 <h1 class="text-2xl font-semibold">Staff workspace</h1><p>Use your ordinary account for the instance work you are authorized to do.</p>
 {#if pending}<p role="status">Checking your current authority…</p>{/if}{#if error}<p role="alert">{error}</p><button class="rounded border p-2" onclick={load}>Try again</button>{/if}
 {#if authority}<p>Signed in as <strong>{authority.username}</strong>.</p><div class="grid gap-4 md:grid-cols-2">
  {#if can(authority,'report.update')}<a class="rounded border p-4 block" href="/staff/reports"><strong>Moderator queue</strong><p>Review site reports, claim work and explain decisions.</p></a>{/if}
  {#if admin}<a class="rounded border p-4 block" href="/staff/users"><strong>People and access</strong><p>Review role grants, restrictions, active sessions and security history.</p></a>{/if}
  {#if can(authority,'operations.read',true)}<a class="rounded border p-4 block" href="/staff/operations"><strong>Operations overview</strong><p>Read service health and unresolved work without replaying commands.</p></a>{/if}
  {#if can(authority,'user.update',true)}<a class="rounded border p-4 block" href="/users"><strong>Public profiles</strong><p>Find a person, then use the staff editor from their profile settings.</p></a>{/if}
  <a class="rounded border p-4 block" href="/my/reports"><strong>My reports</strong><p>Only your own submissions and appeals, regardless of your staff role.</p></a>
 </div><details><summary>Your current instance authority</summary><p>Roles: {authority.roles.join(', ')||'ordinary account'}</p><ul>{#each authority.permissions as permission}<li>{permission}</li>{/each}</ul><p>These grants do not confer original-service operator or publishing authority.</p></details>{:else if !$Session?.identity?.id}<a class="underline" href="/auth/login">Sign in</a>{/if}
 <p>For original-service publisher, admission, peer or storage work, use the separately authorized operator procedure. Never paste operator credentials or private keys into this workspace.</p>
</section>
