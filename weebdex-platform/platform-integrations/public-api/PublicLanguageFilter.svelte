<script lang="ts">
 import { Locale } from '$lib';
 import { searchLanguageCodes } from './language';
 let {value=$bindable(),label,disabled=false}:{value:string[]|undefined;label:string;disabled?:boolean}=$props();
 const instanceId=$props.id();
 const hintId=(code:string)=>`${instanceId}-${code}-hint`;
 let filter=$state('');
 const names=Locale.Languages as Record<string,string>;
 // Search aliases describe the same language, not two separate choices. Do not
 // rewrite an incoming URL or saved preference until the user edits this field.
 const choice=(code:string)=>code==='tam'?'ta':code;
 const codes=(code:string)=>searchLanguageCodes([code]);
 const name=(code:string)=>choice(code)==='ta'?'Tamil':names[code]??code;
 const partial=(code:string)=>codes(code).some(item=>value?.includes(item))&&!codes(code).every(item=>value?.includes(item));
 const partialLabel=(code:string)=>partial(code)?'some entries only':'';
 const selected=$derived([...new Set((value??[]).map(choice))]);
 const options=$derived([...new Set([...Object.keys(names),...(value??[])].map(choice))].filter(code=>`${name(code)} ${codes(code).join(' ')}`.toLowerCase().includes(filter.toLowerCase())));
 function toggle(code:string,checked:boolean){const aliases=codes(code);value=checked?[...new Set([...(value??[]),...aliases])]:(value??[]).filter(item=>!aliases.includes(item));}
</script>
<details class="min-w-50 flex-auto rounded border p-2">
 <summary class="cursor-pointer" aria-disabled={disabled}>{label}: {selected.length ? selected.map(code=>name(code)+(partial(code)?` (${partialLabel(code)})`:'')).join(', ') : 'Any'}</summary>
 <fieldset {disabled} class="mt-2 space-y-2">
  <legend class="sr-only">{label}</legend>
  <input class="block w-full rounded border p-2" aria-label={'Find '+label.toLowerCase()} bind:value={filter} placeholder="Find a language" />
  <button type="button" class="text-sm underline" onclick={()=>{value=[];filter='';}}>Clear selection</button>
  <div class="max-h-48 overflow-y-auto">
   {#each options as code}
    <div class="p-1">
     <label class="flex items-center gap-2"><input type="checkbox" aria-label={name(code)} aria-describedby={partial(code)?hintId(code):undefined} checked={codes(code).every(item=>value?.includes(item))} indeterminate={partial(code)} onchange={event=>toggle(code,event.currentTarget.checked)} />{name(code)}</label>
     {#if partial(code)}<p id={hintId(code)} class="text-muted-foreground text-sm">Tamil: {partialLabel(code)}. Select Tamil to include all Tamil entries.</p>{/if}
    </div>
   {/each}
   {#if !options.length}<p class="text-sm">No languages found.</p>{/if}
  </div>
 </fieldset>
</details>
