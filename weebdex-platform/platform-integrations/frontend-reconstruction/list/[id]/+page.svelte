<script lang="ts">import {page} from "$app/state";import PublicListPage from "$publicApi/PublicListPage.svelte";</script>
<PublicListPage id={page.params.id??""}/>
