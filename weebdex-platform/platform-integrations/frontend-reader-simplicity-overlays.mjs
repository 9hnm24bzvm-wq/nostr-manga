// Fresh-reader presentation only. No mutation authority or saved preference migration.
// Composed after the reviewed overlays, with original-source hash and exact anchor checks.
export const readerSimplicityPatches = [
  {
    "path": "src/lib/stores/prefs.ts",
    "label": "fresh_reader_starts_with_settings_closed",
    "sha256": "cd441d7ca0bddd4ac0e2c8d60b40f073143b0c3b47084455dc2bb2c80acb64b6",
    "count": 2,
    "before": "\tpanel: !isMobile(),",
    "after": "\tpanel: false,",
    "approved_reader_simplicity_exception": "fresh_reader_starts_with_settings_closed"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_advanced_controls_are_optional",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Separator />\r\n<div class=\"p-2\">",
    "after": "<details class=\"border-t\">\r\n<summary class=\"cursor-pointer p-3 font-medium\">More reading options</summary>\r\n<p class=\"text-muted-foreground px-3 pb-2 text-sm\">Fine-tune page spacing, navigation and loading. Your saved choices stay active when this section is closed.</p>\r\n<div class=\"p-2\">",
    "approved_reader_simplicity_exception": "reader_advanced_controls_are_optional"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_advanced_disclosure_ends_before_library_link",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Separator />\r\n<div class=\"space-y-2 p-2\">\r\n\t<Button class=\"w-full border-none\" variant=\"outline\" href=\"/settings/library\">",
    "after": "</details>\r\n<Separator />\r\n<div class=\"space-y-2 p-2\">\r\n\t<Button class=\"w-full border-none\" variant=\"outline\" href=\"/settings/library\">",
    "approved_reader_simplicity_exception": "reader_advanced_disclosure_ends_before_library_link",
    "composed_before": true
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_width",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Maximum Page Width",
    "after": "Page width",
    "approved_reader_simplicity_exception": "reader_friendly_width"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_width",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.width}",
    "after": "<Slider aria-label=\"Page width\" type=\"single\" bind:value={$readerPrefs.width}",
    "approved_reader_simplicity_exception": "reader_accessible_width"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_gap",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Long Strip Gap",
    "after": "Space between pages",
    "approved_reader_simplicity_exception": "reader_friendly_gap"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_gap",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.gap}",
    "after": "<Slider aria-label=\"Space between pages\" type=\"single\" bind:value={$readerPrefs.gap}",
    "approved_reader_simplicity_exception": "reader_accessible_gap"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_offset",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Spread Offset",
    "after": "Start paired pages after",
    "approved_reader_simplicity_exception": "reader_friendly_offset"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_offset",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.offset}",
    "after": "<Slider aria-label=\"Start paired pages after\" type=\"single\" bind:value={$readerPrefs.offset}",
    "approved_reader_simplicity_exception": "reader_accessible_offset"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_preload",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Maximum Preload Range",
    "after": "Pages to load ahead",
    "approved_reader_simplicity_exception": "reader_friendly_preload"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_preload",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.preload}",
    "after": "<Slider aria-label=\"Pages to load ahead\" type=\"single\" bind:value={$readerPrefs.preload}",
    "approved_reader_simplicity_exception": "reader_accessible_preload"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_parallel",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Maximum Parallel Downloads",
    "after": "Images loading at once",
    "approved_reader_simplicity_exception": "reader_friendly_parallel"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_parallel",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.parallel}",
    "after": "<Slider aria-label=\"Images loading at once\" type=\"single\" bind:value={$readerPrefs.parallel}",
    "approved_reader_simplicity_exception": "reader_accessible_parallel"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_friendly_scroll",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "Keyboard Scroll Speed",
    "after": "Keyboard scroll distance",
    "approved_reader_simplicity_exception": "reader_friendly_scroll"
  },
  {
    "path": "src/lib/components/reader/reader-settings.svelte",
    "label": "reader_accessible_scroll",
    "sha256": "414271f285f269a15f9f171d41e11c96456cf498aa63805916728b7de6e54b90",
    "count": 1,
    "before": "<Slider type=\"single\" bind:value={$readerPrefs.scroll}",
    "after": "<Slider aria-label=\"Keyboard scroll distance\" type=\"single\" bind:value={$readerPrefs.scroll}",
    "approved_reader_simplicity_exception": "reader_accessible_scroll"
  },
  {
    "path": "src/lib/components/reader/reader-pagination.svelte",
    "label": "reader_settings_has_visible_label",
    "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
    "count": 1,
    "before": "<Button class=\"border-none\" variant=\"outline\" size=\"icon-lg\" onclick={sidebar.toggle}",
    "after": "<Button class=\"border-none px-3\" variant=\"outline\" onclick={sidebar.toggle}",
    "approved_reader_simplicity_exception": "reader_settings_has_visible_label"
  },
  {
    "path": "src/lib/components/reader/reader-pagination.svelte",
    "label": "reader_settings_visible_text",
    "sha256": "c600e78d8754207a5a4b791458c36ef7e3bdcd7902e53d55eb81f702f764d36b",
    "count": 1,
    "before": "<SettingsIcon class=\"size-5\" />",
    "after": "<SettingsIcon class=\"size-5\" />\r\n\t\t\t\t\t<span>Settings</span>",
    "approved_reader_simplicity_exception": "reader_settings_visible_text"
  },
  {
    "path": "src/lib/components/comments/comments.svelte",
    "label": "comment_sign_in_preserves_return_route",
    "sha256": "09c5e198006e3e6f6b12821edb467e8446b29de8fe0c4e2a0627846335a8648f",
    "count": 1,
    "before": "href=\"/auth/login{new URLSearchParams({\r\n\t\t\t\t\t\tredirect: page.url.href\r\n\t\t\t\t\t})}\">Login</a",
    "after": "href={'/auth/login?redirect=' + encodeURIComponent(page.url.href)}>Sign in</a",
    "approved_reader_simplicity_exception": "comment_sign_in_preserves_return_route"
  },
  {
    "path": "src/lib/components/comments/comments.svelte",
    "label": "comment_create_account_preserves_return_route",
    "sha256": "09c5e198006e3e6f6b12821edb467e8446b29de8fe0c4e2a0627846335a8648f",
    "count": 1,
    "before": "href=\"/auth/register{new URLSearchParams({\r\n\t\t\t\t\t\tredirect: page.url.href\r\n\t\t\t\t\t})}\">Register</a",
    "after": "href={'/auth/register?redirect=' + encodeURIComponent(page.url.href)}>Create account</a",
    "approved_reader_simplicity_exception": "comment_create_account_preserves_return_route"
  }
];
