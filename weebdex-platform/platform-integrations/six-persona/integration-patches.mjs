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
},{
 path:'src/lib/components/ChapterFeed.svelte',label:'account-history-empty-explains-completion-only',
 before:'<p>The database may still be updating, or you haven\'t read anything yet.</p>\r\n\t\t\t\t\t\t<br />\r\n\t\t\t\t\t\t<p>Any chapters you read will appear here shortly.</p>',
 after:'<p>History shows completed chapters, not every page you have opened.</p>\r\n\t\t\t\t\t\t<br />\r\n\t\t\t\t\t\t<p>Finish a chapter to add it to History. If you have already finished chapters, check your filters.</p>',count:1
},{
  "path": "src/routes/(app)/auth/logout/+page.svelte",
  "label": "native-logout-origin-bearing-post-and-confirmed-session-exit",
  "before": "<script lang=\"ts\">\r\n\timport { goto } from \"$app/navigation\";\r\n\timport { page } from \"$app/state\";\r\n\timport { createAuthFormOwner } from \"$platform/auth-form-owner\";\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport { Constants, Kratos } from \"$lib\";\r\n\timport * as Empty from \"$lib/components/ui/empty\";\r\n\timport { Spinner } from \"$lib/components/ui/spinner\";\r\n\timport { Session } from \"$lib/stores\";\r\n\timport { cn } from \"$lib/Utils\";\r\n\timport { Check } from \"@lucide/svelte\";\r\n\timport { onMount } from \"svelte\";\r\n\timport { toast } from \"svelte-sonner\";\r\n\r\n\tconst authFormOwner = createAuthFormOwner();\r\n\t$effect.pre(() => { page.url.href; authFormOwner.observe(window.location.href); });\r\n\tlet isMounted = $state(false);\r\n\tonMount(async () => {\r\n\t\tconst authIntent = authFormOwner.capture();\r\n\t\tconst redirect = decodeURIComponent(page.url.searchParams.get(\"redirect\") ?? \"/\");\r\n\t\tconst signedIn = await Session.isValid();\r\n\t\tif (!authIntent.alive()) return;\r\n\t\tif (!signedIn) {\r\n\t\t\treturn goto(redirect);\r\n\t\t}\r\n\t\tisMounted = true;\r\n\r\n\t\ttry {\r\n\t\t\tconst res = await Kratos.createBrowserLogoutFlow();\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (res.data) {\r\n\t\t\t\tawait Kratos.updateLogoutFlow({ token: res.data.logout_token });\r\n\t\t\t\tif (!authIntent.alive()) return;\r\n\t\t\t\t$Session = undefined;\r\n\r\n\t\t\t\tgoto(redirect);\r\n\t\t\t\ttoast.success(\"You have been logged out\");\r\n\t\t\t}\r\n\t\t} catch (e) {\r\n\t\t\tif (!authIntent.alive()) return;\r\n\t\t\tif (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });\r\n\t\t\telse {\r\n\t\t\t\tconsole.error(e);\r\n\t\t\t\ttoast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });\r\n\t\t\t}\r\n\t\t}\r\n\t});\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>Logout - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n{#if isMounted}\r\n\t<Empty.Root>\r\n\t\t<Empty.Header>\r\n\t\t\t<Empty.Media>\r\n\t\t\t\t<Check class={cn(\"size-10 scale-100 transition-all\", $Session && \"scale-0\")} />\r\n\t\t\t\t<Spinner class={cn(\"absolute size-10 scale-0 transition-all\", $Session && \"scale-100 \")} />\r\n\t\t\t</Empty.Media>\r\n\t\t\t<Empty.Title>Logging out...</Empty.Title>\r\n\t\t</Empty.Header>\r\n\t</Empty.Root>\r\n{/if}\r\n",
  "after": "<script lang=\"ts\">import NativeLogout from '$platform/NativeLogout.svelte';</script>\n<NativeLogout />\n",
  "count": 1
}];
export const sixPersonaRouteAdditions=[
 ...['+page.svelte','reports/+page.svelte','operations/+page.svelte','users/+page.svelte','users/[id]/profile/+page.svelte'].map(name=>({source:'platform-integrations/frontend-reconstruction/staff/'+name,target:'src/routes/(app)/staff/'+name,reconstruction:false,additive_six_persona_route:true})),
 {source:'platform-integrations/frontend-reconstruction/upload/+page.svelte',target:'src/routes/(app)/upload/+page.svelte',reconstruction:false,additive_six_persona_route:true},
 {source:'platform-integrations/frontend-reconstruction/teams/+page.svelte',target:'src/routes/(app)/teams/+page.svelte',reconstruction:false,additive_six_persona_route:true}
];
export const sixPersonaComponents=['frontend-behavior/NativeLogout.svelte','local-library/RemoteSignerPanel.svelte','local-library/RemoteSignerStatus.svelte','local-library/NativeIdentityLink.svelte','public-api/UnsavedEditorGuard.svelte','public-api/PrivateDraftPreview.svelte','public-api/NativeChapterCreditsEditor.svelte','public-api/NativeGroupTeam.svelte','public-api/UploadWorkbench.svelte','public-api/PublicChapterCredits.svelte','public-api/NativeGroupWorkspace.svelte',...['StaffAdministration','OperationsWorkspace','InstancePolicy','PersonalReportHistory','ProfileSettingsEditor','StaffProfileEditor','StaffWorkspace','ModerationWorkspace'].map(name=>'staff-workspace/'+name+'.svelte')];
