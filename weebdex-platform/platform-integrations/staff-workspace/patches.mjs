// Exact final-stage successors of the eight-stage composed baseline.
export const staffWorkspacePatches = [
  {
    "path": "src/routes/(app)/user/[id]/settings/+page.svelte",
    "label": "dedicated staff-aware native profile selection",
    "before": "import AccountProfileEditor from \"$publicApi/AccountProfileEditor.svelte\";",
    "after": "import AccountProfileEditor from \"$staffWorkspace/ProfileSettingsEditor.svelte\";",
    "count": 1
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "personal report history component import",
    "before": "<script lang=\"ts\">",
    "after": "<script lang=\"ts\">\r\n\timport PersonalReportHistory from \"$staffWorkspace/PersonalReportHistory.svelte\";",
    "count": 1
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "personal reports cannot conceal site-wide mutation actions",
    "before": "\t\t\t\t\t\t{#if Permission.Report.canDelete()}\r\n\t\t\t\t\t\t\t<ConfirmDialog\r\n\t\t\t\t\t\t\t\tvariant=\"destructive\"\r\n\t\t\t\t\t\t\t\ttitle=\"Delete Report\"\r\n\t\t\t\t\t\t\t\tdescription=\"Are you sure you want to delete this report?\"\r\n\t\t\t\t\t\t\t\tconfirmText=\"Delete\"\r\n\t\t\t\t\t\t\t\tonConfirm={m => m.exec(deleteReport)}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex}\r\n\t\t\t\t\t\t\t>\r\n\t\t\t\t\t\t\t\t{#snippet child({ props })}\r\n\t\t\t\t\t\t\t\t\t<Button class=\"w-24\" variant=\"destructive\" {...props}>\r\n\t\t\t\t\t\t\t\t\t\t{#if $mutex && isDeleting}\r\n\t\t\t\t\t\t\t\t\t\t\t<Spinner />\r\n\t\t\t\t\t\t\t\t\t\t{:else}\r\n\t\t\t\t\t\t\t\t\t\t\t<TrashIcon />\r\n\t\t\t\t\t\t\t\t\t\t\tDelete\r\n\t\t\t\t\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t\t\t\t</Button>\r\n\t\t\t\t\t\t\t\t{/snippet}\r\n\t\t\t\t\t\t\t</ConfirmDialog>\r\n\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t{#if report.status === Constants.Report.Status.Waiting && Permission.Report.canUpdate()}\r\n\t\t\t\t\t\t\t<TextAreaDialog\r\n\t\t\t\t\t\t\t\tvariant=\"destructive\"\r\n\t\t\t\t\t\t\t\ttitle=\"Refuse Report\"\r\n\t\t\t\t\t\t\t\tdescription=\"Are you sure you want to refuse this report?\"\r\n\t\t\t\t\t\t\t\tplaceholder=\"Reason (Optional)\"\r\n\t\t\t\t\t\t\t\tconfirmText=\"Refuse\"\r\n\t\t\t\t\t\t\t\tonConfirm={(m, notes) => m.exec(() => updateStatus(Constants.Report.Status.Refused, notes))}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex}\r\n\t\t\t\t\t\t\t>\r\n\t\t\t\t\t\t\t\t{#snippet child({ props })}\r\n\t\t\t\t\t\t\t\t\t<Button class=\"w-24 border-none !text-red-400\" variant=\"outline\" {...props}>\r\n\t\t\t\t\t\t\t\t\t\t{#if $mutex && mutexState === Constants.Report.Status.Refused}\r\n\t\t\t\t\t\t\t\t\t\t\t<Spinner />\r\n\t\t\t\t\t\t\t\t\t\t{:else}\r\n\t\t\t\t\t\t\t\t\t\t\t<XIcon />\r\n\t\t\t\t\t\t\t\t\t\t\tRefuse\r\n\t\t\t\t\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t\t\t\t</Button>\r\n\t\t\t\t\t\t\t\t{/snippet}\r\n\t\t\t\t\t\t\t</TextAreaDialog>\r\n\t\t\t\t\t\t\t<TextAreaDialog\r\n\t\t\t\t\t\t\t\tvariant=\"default\"\r\n\t\t\t\t\t\t\t\ttitle=\"Accept Report\"\r\n\t\t\t\t\t\t\t\tdescription=\"Are you sure you want to accept this report?\"\r\n\t\t\t\t\t\t\t\tplaceholder=\"Reason (Optional)\"\r\n\t\t\t\t\t\t\t\tconfirmText=\"Accept\"\r\n\t\t\t\t\t\t\t\tonConfirm={(m, notes) => m.exec(() => updateStatus(Constants.Report.Status.Accepted, notes))}\r\n\t\t\t\t\t\t\t\tdisabled={$mutex}\r\n\t\t\t\t\t\t\t>\r\n\t\t\t\t\t\t\t\t{#snippet child({ props })}\r\n\t\t\t\t\t\t\t\t\t<Button class=\"w-24 border-none !text-green-400\" variant=\"outline\" {...props}>\r\n\t\t\t\t\t\t\t\t\t\t{#if $mutex && mutexState === Constants.Report.Status.Accepted}\r\n\t\t\t\t\t\t\t\t\t\t\t<Spinner />\r\n\t\t\t\t\t\t\t\t\t\t{:else}\r\n\t\t\t\t\t\t\t\t\t\t\t<CheckIcon />\r\n\t\t\t\t\t\t\t\t\t\t\tAccept\r\n\t\t\t\t\t\t\t\t\t\t{/if}\r\n\t\t\t\t\t\t\t\t\t</Button>\r\n\t\t\t\t\t\t\t\t{/snippet}\r\n\t\t\t\t\t\t\t</TextAreaDialog>\r\n\t\t\t\t\t\t{/if}\r\n",
    "after": "\t\t\t\t\t\t{#if Permission.Report.canUpdate() || Permission.Report.canDelete()}<a class=\"underline\" href=\"/staff/reports\">Open Moderator queue</a>{/if}\r\n",
    "count": 1
  },
  {
    "path": "src/lib/components/ReportSearchEntry.svelte",
    "label": "personal reasoned appeal and history entry point",
    "before": "\t\t\t\t\t<Dialog.Footer>",
    "after": "\t\t\t\t\t<PersonalReportHistory reportId={report.id}/>\r\n\t\t\t\t\t<Dialog.Footer>",
    "count": 1
  },
  {
    "path": "src/routes/(app)/my/reports/+page.svelte",
    "label": "personal report scope copy",
    "before": "<h2 class=\"text-xl font-medium\">My Reports</h2>",
    "after": "<h2 class=\"text-xl font-medium\">My Reports</h2>\r\n\t\t<p>Only reports you submitted on this instance appear here, even if you are a moderator. Open a report for its decision history and appeal options.</p>",
    "count": 1
  },
  {
    "path": "src/lib/components/AppSidebar.svelte",
    "label": "normal role-aware staff navigation",
    "before": "items: [{ name: \"Home\", url: \"/\", icon: HomeIcon }]",
    "after": "items: [{ name: \"Home\", url: \"/\", icon: HomeIcon },\r\n\t\t\t\t{ name: \"Moderator queue\", url: \"/staff/reports\", icon: FlagIcon, hidden: !$Session?.roles?.some(role => [\"moderator\", \"staff\", \"sysop\"].includes(role)) || !$Session?.permissions?.includes(\"report.update\") },\r\n\t\t\t\t{ name: \"Staff workspace\", url: \"/staff\", icon: SettingsIcon, hidden: !$Session?.roles?.some(role => [\"moderator\", \"staff\", \"sysop\"].includes(role)) || !$Session?.permissions?.some(permission => [\"report.update\", \"report.history\", \"user.update\", \"operations.read\", \"admin.users.read\", \"admin.roles.manage\", \"admin.restrictions.manage\", \"admin.sessions.revoke\", \"admin.audit.read\"].includes(permission)) }]",
    "count": 1
  },
  {
    "path": "src/routes/(app)/about/+page.svelte",
    "label": "configured instance about instead of upstream policy",
    "before": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>About - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n<section class=\"max-w-200 space-y-5\">\r\n\t<header>\r\n\t\t<h2 class=\"text-2xl font-semibold\">About</h2>\r\n\t\t<span class=\"text-foreground/85\">Last Revised: February 10, 2026</span>\r\n\t</header>\r\n\t<div class=\"space-y-5 text-base\">\r\n\t\t<div class=\"typography space-y-2\">\r\n\t\t\t<p>\r\n\t\t\t\tWeebDex is an ad-free, scanlator-first online manga reader that delivers high-quality, zero-compression images.\r\n\t\t\t</p>\r\n\t\t\t<p>\r\n\t\t\t\tOur primary goal is to preserve <strong>manga scanlations</strong> and provide a platform where scanlators can share\r\n\t\t\t\ttheir work without worrying about DMCA takedowns. We strive to ensure these works don't disappear at the whim of large\r\n\t\t\t\tcompanies.\r\n\t\t\t</p>\r\n\t\t\t<p>\r\n\t\t\t\tWe welcome all scanlators, regardless of country of origin or language. Scanlators are free to upload their work\r\n\t\t\t\tto WeebDex and retain full control over their releases. WeebDex generally does not police content or scanlators\r\n\t\t\t\tunless they violate our Terms of Service.\r\n\t\t\t</p>\r\n\t\t\t<p>\r\n\t\t\t\tThe site is still in its early stages, and we are working hard to deliver a better experience for our users.\r\n\t\t\t</p>\r\n\t\t\t<p>\r\n\t\t\t\tWe also provide OpenAPI specifications for our API, which you can find <a href=\"https://api.weebdex.org/docs\"\r\n\t\t\t\t\t>here</a\r\n\t\t\t\t>.\r\n\t\t\t</p>\r\n\t\t</div>\r\n\t</div>\r\n</section>\r\n",
    "after": "<script lang=\"ts\">import InstancePolicy from '$staffWorkspace/InstancePolicy.svelte';</script>\n<InstancePolicy kind=\"about\"/>\n",
    "count": 1
  },
  {
    "path": "src/routes/(app)/terms/+page.svelte",
    "label": "configured instance terms instead of upstream policy",
    "before": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>Terms of Service - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n<section class=\"max-w-200 space-y-5\">\r\n\t<header>\r\n\t\t<h2 class=\"text-2xl font-semibold\">Terms of Service</h2>\r\n\t\t<span class=\"text-foreground/85\">Last Revised: December 2, 2025</span>\r\n\t</header>\r\n\t<div class=\"space-y-5 text-base\">\r\n\t\t<div class=\"typography space-y-2\">\r\n\t\t\t<p>\r\n\t\t\t\tBy using this site, you agree to these Terms. You must be at least 13 years old (or the minimum age in your\r\n\t\t\t\tcountry) to use the site. Some features require an account; you are responsible for all activity on your\r\n\t\t\t\taccount, must keep your credentials secure, and we may remove or change usernames that are inappropriate. Do not\r\n\t\t\t\tupload child pornography or child abuse material, attempt unauthorized access, or interfere with the site's\r\n\t\t\t\tfunctionality. By submitting content (titles, chapters, comments, reviews, etc.), you grant us a license to\r\n\t\t\t\thost, display, and distribute it. We may remove any content at our discretion if it violates these Terms.\r\n\t\t\t</p>\r\n\t\t\t<p>We may update these Terms at any time; continued use constitutes acceptance of the updated Terms.</p>\r\n\t\t\t<p>\r\n\t\t\t\tIf you have any questions regarding this Terms of Service, please contact us at:\r\n\t\t\t\t<a href=\"mailto:system@weebdex.org\">system@weebdex.org</a>\r\n\t\t\t</p>\r\n\t\t</div>\r\n\t</div>\r\n</section>\r\n",
    "after": "<script lang=\"ts\">import InstancePolicy from '$staffWorkspace/InstancePolicy.svelte';</script>\n<InstancePolicy kind=\"terms\"/>\n",
    "count": 1
  },
  {
    "path": "src/routes/(app)/privacy/+page.svelte",
    "label": "configured instance privacy instead of upstream policy",
    "before": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>Privacy Policy - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n<section class=\"max-w-200 space-y-5\">\r\n\t<header>\r\n\t\t<h2 class=\"text-2xl font-semibold\">Privacy Policy</h2>\r\n\t\t<span class=\"text-foreground/85\">Last Revised: October 4, 2025</span>\r\n\t</header>\r\n\t<div class=\"space-y-5 text-base\">\r\n\t\t<div class=\"typography space-y-2\">\r\n\t\t\t<p>\r\n\t\t\t\tBy using this site, you consent to this policy. We respect your privacy and collect only the information you\r\n\t\t\t\tvoluntarily provide, such as email, username, and password. We do not sell, trade, or share your information,\r\n\t\t\t\texcept in cases of serious violations of our Terms of Service, such as uploading or distributing child\r\n\t\t\t\tpornography. Links to third-party sites may appear on our website, but we are not responsible for their content\r\n\t\t\t\tor privacy practices.\r\n\t\t\t</p>\r\n\t\t\t<p>\r\n\t\t\t\tIf you have any questions regarding this Privacy Policy, please contact us at:\r\n\t\t\t\t<a href=\"mailto:system@weebdex.org\">system@weebdex.org</a>\r\n\t\t\t</p>\r\n\t\t</div>\r\n\t</div>\r\n</section>\r\n",
    "after": "<script lang=\"ts\">import InstancePolicy from '$staffWorkspace/InstancePolicy.svelte';</script>\n<InstancePolicy kind=\"privacy\"/>\n",
    "count": 1
  },
  {
    "path": "src/routes/(app)/guidelines/+page.svelte",
    "label": "configured instance guidelines instead of upstream policy",
    "before": "<script lang=\"ts\">\r\n\timport { PUBLIC_SITE_NAME } from \"$env/static/public\";\r\n\timport Guidelines from \"$lib/components/Guidelines.svelte\";\r\n</script>\r\n\r\n<svelte:head>\r\n\t<title>Guidelines - {PUBLIC_SITE_NAME}</title>\r\n</svelte:head>\r\n\r\n<section class=\"space-y-5\">\r\n\t<header>\r\n\t\t<h2 class=\"text-2xl font-semibold\">Guidelines</h2>\r\n\t\t<span class=\"text-foreground/85\">Last Revised: January 11, 2026</span>\r\n\t</header>\r\n\t<div class=\"space-y-5 text-base\">\r\n\t\t<Guidelines />\r\n\t</div>\r\n</section>\r\n",
    "after": "<script lang=\"ts\">import InstancePolicy from '$staffWorkspace/InstancePolicy.svelte';</script>\n<InstancePolicy kind=\"guidelines\"/>\n",
    "count": 1
  }
];
