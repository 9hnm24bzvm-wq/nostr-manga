// Build-local routes. Supplied frontend files remain byte-identical.
export const frontendRouteAdditions = [
  ...["manga", "group", "user"].map(kind=>({
    source: `platform-integrations/frontend-reconstruction/following/${kind}/+page.svelte`,
    target: `src/routes/(app)/following/${kind}/+page.svelte`,
    reconstruction: false,
    additive_account_following_route: true,
  })),
  ...["list/[id]", "user/[id]/lists", "following/lists"].map(route=>({
    source: `platform-integrations/frontend-reconstruction/${route}/+page.svelte`,
    target: `src/routes/(app)/${route}/+page.svelte`,
    reconstruction: false,
    additive_public_list_route: true,
  })),
  ...['+page.svelte', '[id]/+page.svelte', 'manage/+page.svelte'].map(name => ({
    source: 'platform-integrations/frontend-reconstruction/announcements/routes/' + name,
    target: 'src/routes/(app)/announcements/' + name,
    reconstruction: true,
  })),
  {
    source: 'platform-integrations/frontend-reconstruction/settings/identity/+page.svelte',
    target: 'src/routes/(app)/settings/identity/+page.svelte',
    reconstruction: false,
    additive_settings_route: true,
  },
];
