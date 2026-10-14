// Build-local routes. Supplied frontend files remain byte-identical.
export const frontendRouteAdditions = [
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
