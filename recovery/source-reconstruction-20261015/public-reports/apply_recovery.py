#!/usr/bin/env python3
"""Apply only reconstructed report source and minimal wiring; do not verify it."""
from pathlib import Path
import sys

if len(sys.argv) != 2:
    raise SystemExit("Usage: python3 apply_recovery.py /absolute/repository/path")
source = Path(__file__).resolve().parent
root = Path(sys.argv[1]).resolve()
backend = root / "weebdex-platform/backend"
if not backend.is_dir():
    raise SystemExit("The selected repository does not contain weebdex-platform/backend")
auth = backend / "src/public_api/auth.rs"
if not auth.is_file() or "BoundAccountWrite" not in auth.read_text():
    raise SystemExit("Restore genuine public API auth and BoundAccountWrite before applying reports")
changes = {}
def read(relative):
    path = root / relative
    if not path.is_file():
        raise SystemExit(f"Restore required foundation file first: {relative}")
    return path.read_text()
def add_once(relative, old, new, guard):
    data = changes.get(relative, read(relative))
    if guard not in data:
        if old not in data:
            raise SystemExit(f"Expected foundation wiring not found in {relative}; review manually")
        data = data.replace(old, new, 1)
    changes[relative] = data

prefix = "weebdex-platform/backend/src/"
add_once(prefix+"public_api/identity.rs", "    Report,", "    Report,\n    ReportReason,", "    ReportReason,")
add_once(prefix+"public_api/identity.rs", '            Self::Report => "report",', '            Self::Report => "report",\n            Self::ReportReason => "report_reason",', 'Self::ReportReason => "report_reason"')
add_once(prefix+"public_api/dto.rs", "    Report,", "    Report,\n    ReportReason,", "    ReportReason,")
add_once(prefix+"public_api/dto.rs", "            Self::Report => ResourceKind::Report,", "            Self::Report => ResourceKind::Report,\n            Self::ReportReason => ResourceKind::ReportReason,", "Self::ReportReason => ResourceKind::ReportReason")
add_once(prefix+"public_api/mod.rs", "pub mod routes;", "pub mod reports;\npub mod routes;", "pub mod reports;")
add_once(prefix+"public_api/routes.rs", ".merge(super::statistics::router())", ".merge(super::statistics::router())\n        .merge(super::reports::router())", ".merge(super::reports::router())")
relative = prefix+"domains/mod.rs"
data = read(relative)
if "0092_public_report_reasons.sql" not in data:
    if "version: 92," in data:
        raise SystemExit("Migration 92 is already occupied; coordinate its number before applying")
    marker = data.rfind("];")
    if marker < 0:
        raise SystemExit("Migration registration array terminator not found")
    data = data[:marker]+'''    Migration {
        version: 92,
        name: "0092_public_report_reasons",
        sql: include_str!("../../migrations/0092_public_report_reasons.sql"),
    },
'''+data[marker:]
changes[relative] = data
relative = prefix+"domains/moderation/mod.rs"
data = read(relative)
if "pub(crate) async fn create_report(" not in data:
    start_marker = "    // Serialize per-user limits and duplicate creation"
    start = data.find(start_marker)
    end = data.find("    tx.commit().await?;", start)
    if start < 0 or end < 0:
        raise SystemExit("Native moderation creation block does not match; compare full module manually")
    core = data[start:end].replace("&a.user_id", "&a.user_id()")
    template = (source/relative).read_text()
    header_start = template.index("// Only genuine native sessions")
    header_end = template.index("    // Serialize per-user limits and duplicate creation", header_start)
    header = template[header_start:header_end]
    data = data[:start]+"    create_report(&tx, &a, &entity, &target, &reason, &details).await?;\n"+data[end:]
    data += "\n"+header+core+"    Ok(id)\n}\n"
changes[relative] = data
for relative in [
    "weebdex-platform/backend/src/public_api/reports.rs",
    "weebdex-platform/backend/migrations/0092_public_report_reasons.sql",
    "weebdex-platform/backend/tests/public_reports.rs",
    "weebdex-platform/docs/public-report-contract.md",
]:
    changes[relative] = (source/relative).read_text()
for relative, data in changes.items():
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(data)
print("Applied reconstructed-unverified report source and wiring. Fresh review and tests remain required.")
