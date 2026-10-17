<script lang="ts">
 import * as Dialog from '$lib/components/ui/dialog';
 import {publicUuid} from './validation';
 let {open=$bindable(),group}:{open:boolean;group:ScanlationGroup}=$props();
 const target=$derived(publicUuid(group.id));
</script>
<Dialog.Header><Dialog.Title>Request local group leadership review</Dialog.Title></Dialog.Header>
<p>This opens the selected signer's instance account and reloads the actual group version before accepting ownership evidence. An instance moderator must explicitly approve the request. No original group key, source role or signing authority changes.</p>
<a class="rounded border px-4 py-2" href={`/catalogue-tools?kind=group&id=${target}`} onclick={()=>{open=false;}}>Continue to signed claim review</a>
