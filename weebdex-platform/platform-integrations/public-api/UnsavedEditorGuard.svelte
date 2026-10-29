<script lang="ts">
 import {beforeNavigate} from '$app/navigation';
 import {guardEditorNavigation} from './editor-unsaved';
 let {dirty}:{dirty:boolean}=$props();
 beforeNavigate(event=>guardEditorNavigation(event,dirty,message=>window.confirm(message)));
</script>
{#if dirty}<p class="text-sm text-muted-foreground" role="status">Unsaved changes — saved only when you choose Save.</p>{/if}
