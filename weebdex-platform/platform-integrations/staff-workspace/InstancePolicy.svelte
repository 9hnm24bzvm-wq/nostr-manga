<script lang="ts">
 import {onMount} from 'svelte';
 let {kind}:{kind:'terms'|'privacy'|'guidelines'|'about'}=$props();
 let information=$state<any>(),error=$state(''),pending=$state(true);
 const title=$derived(({terms:'Terms of service',privacy:'Privacy policy',guidelines:'Community guidelines',about:'About this instance'})[kind]);
 const policy=$derived(kind==='about'?'':information?.[kind]??'');
 const link=$derived(kind==='about'?undefined:information?.[kind+'Url']);
 async function load(signal?:AbortSignal){pending=true;error='';try{const response=await fetch('/api/instance-information',{cache:'no-store',credentials:'omit',redirect:'error',signal});if(!response.ok)throw Error('Instance information is temporarily unavailable.');const result=await response.json();if(result.scope!=='public-instance-information'||typeof result.information?.operatorName!=='string')throw Error('Instance information could not be checked.');information=result.information;}catch(cause){if(!signal?.aborted)error=cause instanceof Error?cause.message:'Instance information could not be loaded.';}finally{if(!signal?.aborted)pending=false;}}
 onMount(()=>{const controller=new AbortController();void load(controller.signal);return()=>controller.abort();});
</script>
<svelte:head><title>{title}</title></svelte:head>
<section class="mx-auto max-w-3xl space-y-5">
 <h1 class="text-2xl font-semibold">{title}</h1>
 {#if pending}<p role="status">Loading this instance’s information…</p>{/if}
 {#if error}<p role="alert">{error}</p><button class="rounded border p-2" onclick={()=>load()}>Try again</button>{/if}
 {#if information}
  <p><strong>Responsible operator:</strong> {information.operatorName||'Not yet configured by this instance’s operator'}</p>
  {#if information.contactUrl}<a class="underline" href={information.contactUrl} rel="noreferrer">Contact this instance’s operator</a>{:else}<p>Operator contact details have not been configured. Do not assume an upstream project operates this site.</p>{/if}
  {#if link}<p><a class="underline" href={link} rel="noreferrer">Read the operator’s {title.toLowerCase()}</a></p>{/if}
  {#if policy}<div class="whitespace-pre-wrap break-words">{policy}</div>{:else if !link&&kind!=='about'}<p>The operator has not supplied this policy. Ask the operator to publish it before relying on this instance’s terms or uploading content.</p>{/if}
  <nav aria-label="Instance policies" class="flex flex-wrap gap-4"><a class="underline" href="/guidelines">Guidelines</a><a class="underline" href="/privacy">Privacy</a><a class="underline" href="/terms">Terms</a><a class="underline" href="/about">About this instance</a></nav>
 {/if}
</section>
