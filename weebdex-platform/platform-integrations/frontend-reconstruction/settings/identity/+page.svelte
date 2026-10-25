<script lang="ts">
  import { PUBLIC_SITE_NAME } from '$env/static/public';
  import PublicIdentitySettings from '$localLibrary/PublicIdentitySettings.svelte';
</script>
<svelte:head><title>Settings: Public identity - {PUBLIC_SITE_NAME}</title></svelte:head>
<PublicIdentitySettings />

<p><a class="underline" href="/reports/moderation">Signed instance moderation</a> requires a currently authorized moderator account, separately from personal report inspection.</p>
