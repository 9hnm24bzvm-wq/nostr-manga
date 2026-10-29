<script lang="ts">
 import * as Dialog from '$lib/components/ui/dialog';
 import {publicUuid} from './validation';
 let {open=$bindable(),group}:{open:boolean;group:ScanlationGroup}=$props();
 const target=$derived(publicUuid(group.id));
</script>
<Dialog.Header><Dialog.Title>Request local group leadership review</Dialog.Title></Dialog.Header>
<p>Continue with your account to review the current group details and provide evidence of ownership. A moderator must approve your request before leadership changes. Submitting does not give you publishing permissions or control of the original publication.</p>
<a class="rounded border px-4 py-2" href={`/catalogue-tools?kind=group&id=${target}`} onclick={()=>{open=false;}}>Continue to leadership review</a>
