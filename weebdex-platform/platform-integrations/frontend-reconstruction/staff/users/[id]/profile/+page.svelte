<script lang="ts">import {page} from '$app/state';import StaffProfileEditor from '$staffWorkspace/StaffProfileEditor.svelte';</script>
<a class="underline" href="/staff">Staff home</a>
<StaffProfileEditor targetId={page.params.id??''}/>
