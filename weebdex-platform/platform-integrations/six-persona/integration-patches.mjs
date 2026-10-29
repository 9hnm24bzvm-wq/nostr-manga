// Integration-owned additions; feature arrays remain separately owned.
export const integrationPatches=[{
 path:'src/lib/components/AppSidebar.svelte',label:'ordinary-account-upload-workbench-entry',
 before:'{ name: "My History", url: "/history", icon: HistoryIcon }',
 after:'{ name: "My History", url: "/history", icon: HistoryIcon }, { name: "Your uploads", url: "/upload", icon: SettingsIcon }',count:1
},{
 path:'src/lib/components/AppHeader.svelte',label:'compact-and-wide-navigation-toggle-accessible-name',
 before:'<Button type="button" variant="ghost" size="icon" onclick={sidebar.toggle}>',
 after:'<Button type="button" variant="ghost" size="icon" aria-label="Toggle navigation" onclick={sidebar.toggle}>',count:1
},{
 path:'src/lib/components/AppHeader.svelte',label:'compact-search-trigger-accessible-name',
 before:'{#if sidebar.isMobile}\r\n\t\t\t\t\t<Dialog.Trigger\r\n',
 after:'{#if sidebar.isMobile}\r\n\t\t\t\t\t<Dialog.Trigger\r\n\t\t\t\t\t\taria-label="Search titles, authors, and groups"\r\n',count:1
},{
 path:'src/lib/components/AppSidebar.svelte',label:'drawer-action-title-and-name-match-current-destination',
 before:'<Sidebar.MenuAction class="!top-1 !right-1 size-6" title="Create Group">',
 after:'<Sidebar.MenuAction class="!top-1 !right-1 size-6" title={item.action.name} aria-label={item.action.name}>',count:1
}];
export const sixPersonaRouteAdditions=[
 ...['+page.svelte','reports/+page.svelte','operations/+page.svelte','users/+page.svelte','users/[id]/profile/+page.svelte'].map(name=>({source:'platform-integrations/frontend-reconstruction/staff/'+name,target:'src/routes/(app)/staff/'+name,reconstruction:false,additive_six_persona_route:true})),
 {source:'platform-integrations/frontend-reconstruction/upload/+page.svelte',target:'src/routes/(app)/upload/+page.svelte',reconstruction:false,additive_six_persona_route:true},
 {source:'platform-integrations/frontend-reconstruction/teams/+page.svelte',target:'src/routes/(app)/teams/+page.svelte',reconstruction:false,additive_six_persona_route:true}
];
export const sixPersonaComponents=['local-library/RemoteSignerPanel.svelte','local-library/RemoteSignerStatus.svelte','local-library/NativeIdentityLink.svelte','public-api/UnsavedEditorGuard.svelte','public-api/NativeChapterCreditsEditor.svelte','public-api/NativeGroupTeam.svelte','public-api/UploadWorkbench.svelte','public-api/PublicChapterCredits.svelte','public-api/NativeGroupWorkspace.svelte',...['StaffAdministration','OperationsWorkspace','InstancePolicy','PersonalReportHistory','ProfileSettingsEditor','StaffProfileEditor','StaffWorkspace','ModerationWorkspace'].map(name=>'staff-workspace/'+name+'.svelte')];
