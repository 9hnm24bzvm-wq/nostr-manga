<script lang="ts">
 import type { Snippet } from 'svelte';
 // Ordinary title creation and editing own their explicit native account check.
 // This layout never grants authority or initializes a separate signing identity.
 let {children}:{children?:Snippet;isAllowed?:boolean}=$props();
</script>
{@render children?.()}
