#!/usr/bin/env python3
"""Record the current complete static generation and its exact bootstrap hashes."""
from pathlib import Path
import base64
import hashlib
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "artifacts/frontend"
EVIDENCE = ROOT / "test-evidence/frontend"
receipt = sys.argv[1] if len(sys.argv) > 1 else "final-build3/build-result.json"
result = json.loads((EVIDENCE / receipt).read_text())
if result["status"] != "passed" or result["command_exit"] != 0 or result["remaining_owned_members"] or result["changed_inputs"]:
    raise SystemExit("A successful frozen-input build terminal receipt is required")
digest = lambda data: hashlib.sha256(data).hexdigest()
integration = json.loads((EVIDENCE / "integration-source-manifest.json").read_text())
build = json.loads((EVIDENCE / "build-integration.json").read_text())
if (integration.get("integration_source_inventory_sha256") != build.get("integration_source_inventory_sha256")
    or integration["overlay_module_sha256"] != build["script_overlay_module_sha256"]
    or integration.get("reader_overlay_sha256") != build.get("reader_overlay_module_sha256")
    or integration.get("local_library_overlay_sha256") != build.get("local_library_overlay_module_sha256")):
    raise SystemExit("Current source integration is newer than built output")
if any(path.is_symlink() for path in UI.rglob("*")):
    raise SystemExit("Static generation must contain regular files only")
files = [{"path": str(path.relative_to(UI)), "bytes": path.stat().st_size, "sha256": digest(path.read_bytes())}
         for path in sorted(UI.rglob("*")) if path.is_file()]
index = (UI / "index.html").read_bytes()
if digest(index) != build["spa_fallback_sha256"]:
    raise SystemExit("Built index differs from integration receipt")
html = index.decode().replace("\r\n", "\n").replace("\r", "\n")
hashes = []
for attributes, body in re.findall(r"<script\b([^>]*)>([\s\S]*?)</script>", html, re.I):
    if (not re.search(r"\bsrc\s*=", attributes, re.I)
            and all(marker in body for marker in ["__sveltekit_", "import(", ".start("])
            and not any(marker in body.lower() for marker in ["googletagmanager", "google-analytics", "gtag(", "g-w464k5049g"])):
        hashes.append("'sha256-" + base64.b64encode(hashlib.sha256(body.encode()).digest()).decode() + "'")
static = []
for path in sorted((ROOT / "frontend/static").rglob("*")):
    if path.is_file():
        target = UI / path.relative_to(ROOT / "frontend/static")
        static.append({"path": str(path.relative_to(ROOT / "frontend/static")), "build_present": target.is_file(),
                       "byte_identical": target.is_file() and path.read_bytes() == target.read_bytes()})
if any(not item["byte_identical"] for item in static):
    raise SystemExit("Supplied static assets changed or missing in output")
manifest = {"ui_directory": "artifacts/frontend", "file_count": len(files), "total_bytes": sum(item["bytes"] for item in files),
            "files": files, "spa_fallback_sha256": digest(index), "bootstrap_csp_hashes": sorted(set(hashes)),
            "source_static_copy_comparison": static, "overlay_module_sha256": integration["overlay_module_sha256"],
            "local_library_overlay_module_sha256": integration.get("local_library_overlay_sha256"),
            "reader_overlay_module_sha256": integration.get("reader_overlay_sha256"),
            "approved_local_protocol_extensions": len(integration.get("approved_local_protocol_extensions", [])),
            "approved_title_context_exceptions": len(integration.get("approved_title_context_exceptions", [])),
            "approved_discussion_context_exceptions": len(integration.get("approved_discussion_context_exceptions", [])),
            "approved_share_control_exceptions": len(integration.get("approved_share_control_exceptions", [])),
            "approved_client_name_exceptions": len(integration.get("approved_client_name_exceptions", [])),
            "approved_search_callback_exceptions": len(integration.get("approved_search_callback_exceptions", [])),
            "approved_auth_flow_key_exceptions": len(integration.get("approved_auth_flow_key_exceptions", [])),
            "approved_create_recovery_exceptions": len(integration.get("approved_create_recovery_exceptions", [])),
            "approved_feed_pagination_exceptions": len(integration.get("approved_feed_pagination_exceptions", [])),
            "approved_widget_error_exceptions": len(integration.get("approved_widget_error_exceptions", [])),
            "approved_reader_selector_exceptions": len(integration.get("approved_reader_selector_exceptions", [])),
            "approved_recommendation_exceptions": len(integration.get("approved_recommendation_exceptions", [])),
            "approved_search_pagination_exceptions": len(integration.get("approved_search_pagination_exceptions", [])),
            "approved_cover_read_exceptions": len(integration.get("approved_cover_read_exceptions", [])),
            "approved_public_carousel_exceptions": len(integration.get("approved_public_carousel_exceptions", [])),
            "anonymous_recovery_derivation": integration.get("anonymous_recovery_derivation"),
            "approved_upload_recovery_exceptions": len(integration.get("approved_upload_recovery_exceptions", [])),
            "approved_recovery_boundary_exceptions": len(integration.get("approved_recovery_boundary_exceptions", [])),
            "integration_source_inventory_sha256": integration.get("integration_source_inventory_sha256"),
            "external_component_extensions": integration.get("external_component_extensions", []),
            "overlay_repairs": len(integration["script_repairs"]), "overlay_original_paths": len(integration["transformed_sources"]),
            "approved_event_handler_exceptions": len(integration["approved_event_handler_exceptions"]),
            "runtime_browser_tests": "Recorded separately; compilation is not browser evidence", "rendered_parity": "unverified"}
(EVIDENCE / "artifact-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
(EVIDENCE / "bootstrap-csp.json").write_text(json.dumps({"ui_index_sha256": digest(index), "hash_sources": manifest["bootstrap_csp_hashes"],
    "recognized_bootstrap_count": len(hashes), "external_analytics_allowed": False, "script_unsafe_inline_allowed": False,
    "runtime_startup_checked": False}, indent=2) + "\n")
status_path = EVIDENCE / "execution-status.json"
status = json.loads(status_path.read_text())
status["frontend_build"].update({"terminal_evidence": receipt, "overlay_repairs": manifest["overlay_repairs"],
    "overlay_original_paths": manifest["overlay_original_paths"], "approved_event_handler_exceptions": manifest["approved_event_handler_exceptions"],
    "spa_fallback_sha256": digest(index), "artifact_is_latest_frozen_overlay": True})
status_path.write_text(json.dumps(status, indent=2) + "\n")
print(json.dumps({key: manifest[key] for key in ["file_count", "total_bytes", "spa_fallback_sha256", "bootstrap_csp_hashes", "overlay_module_sha256", "overlay_repairs", "overlay_original_paths"]}))
