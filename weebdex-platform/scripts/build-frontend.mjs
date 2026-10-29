#!/usr/bin/env node
// Build integration belongs outside the byte-preserved attachment frontend.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { prepareFrontendIntegration } from "./prepare-frontend-integration.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const frontend = process.env.WEEBDEX_FRONTEND_ROOT ?? path.join(root, "frontend");
const env = {
  ...process.env,
  WEEBDEX_FRONTEND_ROOT: frontend,
  PUBLIC_SITE_NAME: process.env.PUBLIC_SITE_NAME ?? "WeebDex",
  PUBLIC_API_URL: process.env.PUBLIC_API_URL ?? "/api",
  PUBLIC_MANGADEX_CATALOGUE: process.env.PUBLIC_MANGADEX_CATALOGUE ?? "0",
  PUBLIC_KRATOS_PUBLIC_API: process.env.PUBLIC_KRATOS_PUBLIC_API ?? "/auth",
  PUBLIC_COVER_URL: process.env.PUBLIC_COVER_URL ?? "/covers",
  PUBLIC_DATA_URL: process.env.PUBLIC_DATA_URL ?? "/data",
  PUBLIC_UPLOAD_URL: process.env.PUBLIC_UPLOAD_URL ?? "/uploads",
  PUBLIC_PROXY_URL: process.env.PUBLIC_PROXY_URL ?? "/proxy",
  PUBLIC_OPENGRAPH_URL: process.env.PUBLIC_OPENGRAPH_URL ?? "/og-image",
};
function run(args) {
  const result = spawnSync(process.execPath, args, { cwd: frontend, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
const integration = prepareFrontendIntegration(frontend);
run([
  path.join(frontend, "node_modules/vite/bin/vite.js"), "build", "--mode", "platform",
  "--config", path.join(root, "scripts/vite.platform.config.mjs"),
]);
const build = path.join(frontend, "build");
const indexPath = path.join(build, "index.html");
const index = fs.readFileSync(indexPath);
// The original post-build script assumes this directory exists and removes the
// SPA fallback. Preserve its Go templates, then restore the exact generated SPA.
fs.mkdirSync(path.join(build, "templates"), { recursive: true });
run([path.join(frontend, "scripts/post_build.js")]);
fs.writeFileSync(indexPath, index);
const portableBuild = path.join(root, "artifacts/frontend");
// Publish only this complete generation, never mix stale hashed entry bundles.
const portableStage = path.join(root, `artifacts/.frontend-${process.pid}`);
if (fs.existsSync(portableStage)) throw new Error("Refusing to reuse an uncertain artifact staging directory");
fs.cpSync(build, portableStage, { recursive: true });
if (fs.existsSync(portableBuild)) {
  fs.mkdirSync(path.join(root, ".build"), { recursive: true });
  fs.renameSync(portableBuild, path.join(root, `.build/frontend-artifact-previous-${Date.now()}`));
}
fs.renameSync(portableStage, portableBuild);
const publicEnv = Object.fromEntries([
  "PUBLIC_SITE_NAME", "PUBLIC_API_URL", "PUBLIC_MANGADEX_CATALOGUE", "PUBLIC_KRATOS_PUBLIC_API", "PUBLIC_COVER_URL",
  "PUBLIC_DATA_URL", "PUBLIC_UPLOAD_URL", "PUBLIC_PROXY_URL", "PUBLIC_OPENGRAPH_URL",
].map(key => [key, env[key]]));
const evidence = {
  six_persona_repairs: integration.six_persona_repairs,
  approved_six_persona_exceptions: integration.approved_six_persona_exceptions,
  six_persona_qualification: integration.six_persona_qualification,
  source_changed: false,
  public_api_catalogue_enabled: integration.public_api_catalogue_enabled,
  public_signed_upload_enabled: integration.public_signed_upload_enabled,
  public_signed_upload_qualification: integration.public_signed_upload_qualification,
  native_account_upload_enabled: integration.native_account_upload_enabled,
  native_account_contribution_enabled: integration.native_account_contribution_enabled,
  native_account_contribution_qualification: integration.native_account_contribution_qualification,
  native_account_upload_repairs: integration.native_account_upload_repairs,
  approved_native_account_upload_exceptions: integration.approved_native_account_upload_exceptions,
  public_signed_upload_overlay_sha256: integration.public_signed_upload_overlay_sha256,
  public_signed_upload_anchors_sha256: integration.public_signed_upload_anchors_sha256,
  approved_public_signed_upload_extensions: integration.approved_public_signed_upload_extensions,
  approved_public_feature_exceptions: integration.approved_public_feature_exceptions,
  approved_public_language_exceptions: integration.approved_public_language_exceptions,
  public_api_overlay_module_sha256: integration.public_api_overlay_sha256,
  public_api_reconstructed_unverified: true,
  qa_build_resource_profile: process.env.WEEBDEX_QA_BUILD_RESOURCE_PROFILE ?? null,
  qa_build_resource_options: process.env.WEEBDEX_QA_BUILD_RESOURCE_PROFILE === "serial-terser" ? { terser_max_workers: 1, report_compressed_size: false, minification_disabled: false, node_options: process.env.NODE_OPTIONS ?? null } : null,
  command: "node scripts/build-frontend.mjs",
  environment: publicEnv,
  spa_fallback_sha256: createHash("sha256").update(index).digest("hex"),
  original_post_build_script_executed: true,
  source_import_case_repaired_externally: ["$lib/utils.js -> src/lib/Utils.ts", "stores/./session -> stores/Session.ts"],
  missing_announcements_reconstructed_externally: true,
  reconstructed_routes: integration.reconstructed_routes,
  integration_utility_sources: integration.integration_utility_sources,
  script_overlay_module_sha256: integration.overlay_module_sha256,
  reader_overlay_module_sha256: integration.reader_overlay_sha256,
  integration_source_inventory_sha256: integration.integration_source_inventory_sha256,
  external_component_extensions: integration.external_component_extensions,
  local_library_overlay_module_sha256: integration.local_library_overlay_sha256,
  approved_local_protocol_extensions: integration.approved_local_protocol_extensions,
  approved_title_context_exceptions: integration.approved_title_context_exceptions,
  approved_discussion_context_exceptions: integration.approved_discussion_context_exceptions,
  approved_share_control_exceptions: integration.approved_share_control_exceptions,
  approved_client_name_exceptions: integration.approved_client_name_exceptions,
  approved_search_callback_exceptions: integration.approved_search_callback_exceptions,
  approved_auth_flow_key_exceptions: integration.approved_auth_flow_key_exceptions,
  approved_registration_onboarding_exceptions: integration.approved_registration_onboarding_exceptions,
  approved_reader_control_exceptions: integration.approved_reader_control_exceptions,
  approved_read_toggle_exceptions: integration.approved_read_toggle_exceptions,
  approved_registration_recovery_exceptions: integration.approved_registration_recovery_exceptions,
  approved_reader_simplicity_exceptions: integration.approved_reader_simplicity_exceptions,
  approved_discovery_copy_exceptions: integration.approved_discovery_copy_exceptions,
  card_spinner_presentation_repairs: integration.card_spinner_presentation_repairs,
  approved_card_spinner_presentation_exceptions: integration.approved_card_spinner_presentation_exceptions,
  profile_presentation_repairs: integration.profile_presentation_repairs,
  approved_profile_presentation_exceptions: integration.approved_profile_presentation_exceptions,
  preferences_presentation_repairs: integration.preferences_presentation_repairs,
  approved_preferences_presentation_exceptions: integration.approved_preferences_presentation_exceptions,
  report_lifecycle_repairs: integration.report_lifecycle_repairs,
  approved_report_lifecycle_exceptions: integration.approved_report_lifecycle_exceptions,
  read_completion_repairs: integration.read_completion_repairs,
  approved_read_completion_exceptions: integration.approved_read_completion_exceptions,
  title_spinner_presentation_repairs: integration.title_spinner_presentation_repairs,
  approved_title_spinner_presentation_exceptions: integration.approved_title_spinner_presentation_exceptions,
  account_library_repairs_qualification: integration.account_library_repairs_qualification,
  account_library_enabled: integration.account_library_enabled,
  account_library_repairs: integration.account_library_repairs,
  approved_account_library_exceptions: integration.approved_account_library_exceptions,
  account_library_overlay_sha256: integration.account_library_overlay_sha256,
  anonymous_recovery_derivation: integration.anonymous_recovery_derivation,
  approved_upload_recovery_exceptions: integration.approved_upload_recovery_exceptions,
  approved_recovery_boundary_exceptions: integration.approved_recovery_boundary_exceptions,
  script_repairs: integration.script_repairs,
  approved_event_handler_exceptions: integration.approved_event_handler_exceptions,
  reconstruction_limit: "Public announcement listing/detail, pinned homepage data, and staff management are additive reconstruction against the new persisted Rust contract. Original missing announcement HTML/CSS and rendered parity remain unknown.",
  portable_ui_directory: "artifacts/frontend",
  original_go_templates_preserved: ["index", "manga", "chapter", "group"].map(name => `templates/${name}.html`),
  visual_parity: "Not asserted. Explicit local/protocol panels, scoped library guards, staging-cancel control, restricted-reset boundary and fresh-registration welcome are intentional UI extensions. Unrelated supplied markup/styles are preserved by exact exception comparison.",
};
const evidenceDir = path.join(root, "test-evidence/frontend");
fs.mkdirSync(evidenceDir, { recursive: true });
fs.writeFileSync(path.join(evidenceDir, "build-integration.json"), JSON.stringify(evidence, null, 2) + "\n");
console.log("Built preserved frontend with a Rust-compatible SPA fallback.");
