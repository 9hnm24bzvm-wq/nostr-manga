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
  if (result.signal) throw new Error(`Frontend build subprocess terminated by signal ${result.signal}`);
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
  "PUBLIC_SITE_NAME", "PUBLIC_API_URL", "PUBLIC_KRATOS_PUBLIC_API", "PUBLIC_COVER_URL",
  "PUBLIC_DATA_URL", "PUBLIC_UPLOAD_URL", "PUBLIC_PROXY_URL", "PUBLIC_OPENGRAPH_URL",
].map(key => [key, env[key]]));
const evidence = {
  source_changed: false,
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
