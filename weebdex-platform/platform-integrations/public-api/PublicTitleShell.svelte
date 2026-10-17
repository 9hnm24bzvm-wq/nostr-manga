<script lang="ts">
 import type { Snippet } from 'svelte';
 // Build-local only for /create/title. PublicTitleEditor checks the selected
 // signer/account; native cookie flags never become public signing authority.
 let {children}:{children?:Snippet;isAllowed?:boolean}=$props();
</script>
{@render children?.()}
