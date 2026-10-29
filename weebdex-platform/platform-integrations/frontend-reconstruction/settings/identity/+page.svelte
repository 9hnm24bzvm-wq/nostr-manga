<script lang="ts">
  import { PUBLIC_SITE_NAME } from '$env/static/public';
  import PublicIdentitySettings from '$localLibrary/PublicIdentitySettings.svelte';
</script>
<svelte:head><title>Settings: Advanced - {PUBLIC_SITE_NAME}</title></svelte:head>
<PublicIdentitySettings />

<details class="mt-4"><summary class="cursor-pointer text-sm">Moderator tools</summary><p class="mt-2 text-sm"><a class="underline" href="/reports/moderation">Review reports</a>. A moderator account is required.</p></details>
