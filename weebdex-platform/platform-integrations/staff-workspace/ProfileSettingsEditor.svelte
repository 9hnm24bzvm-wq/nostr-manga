<script lang="ts">
 import {onDestroy,untrack} from 'svelte';
 import {Session} from '$lib/stores';
 import AccountProfileEditor from '$publicApi/AccountProfileEditor.svelte';
 import {createNativeAccountScope} from '$publicApi/native-account-scope';
 import StaffProfileEditor from './StaffProfileEditor.svelte';
 let {targetId,nativeEligible,onSaved=()=>{}}:{targetId:string;nativeEligible:boolean;onSaved?:(view:any)=>void}=$props();
 let mode=$state<'normal'|'staff'|'checking'>('normal'),error=$state(''),epoch=0,closed=false;
 $effect(()=>{const target=targetId,eligible=nativeEligible,session=$Session;untrack(()=>{const n=++epoch;error='';mode='normal';if(!eligible||!session?.identity?.id||!session.roles?.some(role=>['staff','sysop'].includes(role))||!session.permissions?.includes('user.update'))return;mode='checking';const check=()=>{if(closed||n!==epoch)throw Error('The profile view changed.');};void createNativeAccountScope(session.identity.id,check,{}, {requireVerified:false}).then(scope=>{try{check();mode=scope.account.id===target?'normal':'staff';}finally{scope.dispose();}}).catch(cause=>{try{check();mode='normal';error=cause instanceof Error?cause.message:'Profile authority could not be checked.';}catch{}});});});
 onDestroy(()=>{closed=true;epoch++;});
</script>
{#if error}<p role="alert">{error}</p>{/if}
{#if mode==='checking'}<p role="status">Checking the selected profile…</p>
{:else if mode==='staff'}<StaffProfileEditor {targetId} onSaved={value=>onSaved({...value,resource:{id:value.userId,type:'user',attributes:value.attributes,relationships:[],nostr:{identity:'native_public_user'}}})}/>
{:else}<AccountProfileEditor {targetId} {nativeEligible} {onSaved}/>{/if}
