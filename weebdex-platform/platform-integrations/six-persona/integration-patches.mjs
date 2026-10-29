// Integration-owned additions; feature arrays remain separately owned.
export const integrationPatches=[{
 path:'src/lib/components/AppSidebar.svelte',label:'ordinary-account-upload-workbench-entry',
 before:'{ name: "My History", url: "/history", icon: HistoryIcon }',
 after:'{ name: "My History", url: "/history", icon: HistoryIcon }, { name: "Your uploads", url: "/upload", icon: SettingsIcon }',count:1
}];
export const sixPersonaRouteAdditions=[
 ...['+page.svelte','reports/+page.svelte','operations/+page.svelte','users/+page.svelte','users/[id]/profile/+page.svelte'].map(name=>({source:'platform-integrations/frontend-reconstruction/staff/'+name,target:'src/routes/(app)/staff/'+name,reconstruction:false,additive_six_persona_route:true})),
 {source:'platform-integrations/frontend-reconstruction/upload/+page.svelte',target:'src/routes/(app)/upload/+page.svelte',reconstruction:false,additive_six_persona_route:true},
 {source:'platform-integrations/frontend-reconstruction/teams/+page.svelte',target:'src/routes/(app)/teams/+page.svelte',reconstruction:false,additive_six_persona_route:true}
];
export const sixPersonaComponents=['local-library/NativeIdentityLink.svelte','public-api/UnsavedEditorGuard.svelte','public-api/NativeChapterCreditsEditor.svelte','public-api/NativeGroupTeam.svelte','public-api/UploadWorkbench.svelte','public-api/PublicChapterCredits.svelte','public-api/NativeGroupWorkspace.svelte',...['StaffAdministration','OperationsWorkspace','InstancePolicy','PersonalReportHistory','ProfileSettingsEditor','StaffProfileEditor','StaffWorkspace','ModerationWorkspace'].map(name=>'staff-workspace/'+name+'.svelte')];
