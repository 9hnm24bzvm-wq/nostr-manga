<script lang="ts">
 import { Locale } from '$lib';
 let {value=$bindable(),label,disabled=false}:{value:string[]|undefined;label:string;disabled?:boolean}=$props();
 let filter=$state('');
 const names=Locale.Languages as Record<string,string>;
 const options=$derived([...new Set([...Object.keys(names),...(value??[])])].filter(code=>`${names[code]??code} ${code}`.toLowerCase().includes(filter.toLowerCase())));
 function toggle(code:string,checked:boolean){value=checked?[...new Set([...(value??[]),code])]:(value??[]).filter(item=>item!==code);}
</script>
<details class="min-w-50 flex-auto rounded border p-2">
 <summary class="cursor-pointer" aria-disabled={disabled}>{label}: {value?.length ? value.map(code=>names[code]??code).join(', ') : 'Any'}</summary>
 <fieldset {disabled} class="mt-2 space-y-2">
  <legend class="sr-only">{label}</legend>
  <input class="block w-full rounded border p-2" aria-label={'Find '+label.toLowerCase()} bind:value={filter} placeholder="Find a language" />
  <button type="button" class="text-sm underline" onclick={()=>{value=[];filter='';}}>Clear selection</button>
  <div class="max-h-48 overflow-y-auto">
   {#each options as code}<label class="flex items-center gap-2 p-1"><input type="checkbox" checked={value?.includes(code)??false} onchange={event=>toggle(code,event.currentTarget.checked)} />{names[code]??code}</label>{/each}
   {#if !options.length}<p class="text-sm">No languages found.</p>{/if}
  </div>
 </fieldset>
</details>
