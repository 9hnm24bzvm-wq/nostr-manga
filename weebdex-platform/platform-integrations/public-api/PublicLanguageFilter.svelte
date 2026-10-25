<script lang="ts">
 import { untrack } from 'svelte';
 let {value=$bindable(),label,disabled=false}:{value:string[]|undefined;label:string;disabled?:boolean}=$props();
 let text=$state(untrack(()=>(value??[]).join(', '))),observed=untrack(()=>JSON.stringify(value??[]));
 $effect(()=>{const next=JSON.stringify(value??[]);if(next!==observed){observed=next;text=(value??[]).join(', ');}});
 function edit(event:Event){text=(event.currentTarget as HTMLInputElement).value;const codes=text.split(',').map(code=>code.trim()).filter(Boolean);observed=JSON.stringify(codes);value=codes;}
</script>
<label class="min-w-50 flex-auto space-y-1">{label}
 <input class="block w-full rounded border p-2" value={text} oninput={edit} placeholder="ta, tam, EN" maxlength="1000" {disabled} />
 <span class="text-muted-foreground text-xs">Comma-separated public search codes: 2–5 letters or hyphens; case is preserved.</span>
</label>
