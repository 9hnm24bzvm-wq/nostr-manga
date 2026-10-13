"""Offline exact-ID MangaDex-shaped export to a private local browser backup.

No network, signing key, account or publication is involved. Unmapped data is
reported for review and never silently inferred from titles or chapter numbers.
"""

import argparse
from datetime import datetime, timezone
import json
import time
from src.library_records import library_records, local_records
from src.mangadex_migration import library_preview
from src.operator_json import read_json, save_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input")
    parser.add_argument("output")
    parser.add_argument("--mapping", required=True)
    parser.add_argument("--timestamp", type=int, default=int(time.time()))
    parser.add_argument("--include-private-lists", action="store_true")
    parser.add_argument("--review")
    args = parser.parse_args()
    if not 0 <= args.timestamp <= 2**53 - 1:
        parser.error("timestamp must be a safe browser integer")
    value, mapping = read_json(args.input), read_json(args.mapping)
    if not isinstance(mapping, dict) or set(mapping) - {"series", "chapters"}:
        parser.error("mapping fields must be series and chapters")
    preview = library_preview(
        value,
        mapping.get("series", {}),
        mapping.get("chapters", {}),
    )
    if args.include_private_lists:
        preview["public_lists_for_review"] += preview[
            "private_lists_excluded_pending_review"
        ]
    missing = preview["unmapped"] + [
        dict(kind="list_member", source_id=x)
        for entry in preview["public_lists_for_review"]
        for x in entry["unmapped_series"]
    ]
    report = dict(
        local_only=True,
        publication=False,
        unmapped=missing,
        private_lists_included=args.include_private_lists,
        private_lists_excluded=(
            0
            if args.include_private_lists
            else len(preview["private_lists_excluded_pending_review"])
        ),
    )
    records = local_records(library_records(preview, args.timestamp))
    if args.review:
        save_json(args.review, report, exclusive=True)
    if missing:
        print(json.dumps(report))
        raise SystemExit(2)
    snapshot = dict(
        format="manga-local-library",
        version=1,
        revision=0,
        exported_at=datetime.now(timezone.utc).isoformat(),
        records=records,
    )
    save_json(args.output, snapshot, exclusive=True)
    print(
        json.dumps(
            dict(report, records=sum(map(len, records.values())), output=args.output)
        )
    )


if __name__ == "__main__":
    main()
