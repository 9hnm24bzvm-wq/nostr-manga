// Build-local routes. Supplied frontend files remain byte-identical.
export const frontendRouteAdditions = [
  {source:'platform-integrations/frontend-reconstruction/editorial/+page.svelte',target:'src/routes/(app)/catalogue-tools/+page.svelte',reconstruction:false,additive_editorial_route:true},
  ...[['author-settings-page.ts','author/[id]/settings/+page.ts'],['group-settings-page.ts','group/[id]/settings/+page.ts'],['group-create-page.ts','create/group/+page.ts']].map(([source,target])=>({source:'platform-integrations/frontend-reconstruction/editorial/'+source,target:'src/routes/(app)/'+target,reconstruction:false,additive_editorial_route:true})),
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
