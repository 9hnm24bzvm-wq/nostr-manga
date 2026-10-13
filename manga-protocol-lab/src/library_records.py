"""Shared reviewed-library normalization, independent of persistence or CBOR."""

import uuid


def record_key(family, row):
    return row[0] + row[1] if family == "members" else row[0]


def library_records(preview, timestamp):
    if type(timestamp) is not int or not 0 <= timestamp < 2**63:
        raise ValueError("import timestamp bounds")
    fields = preview["experimental_library_fields"]
    rows = {
        "chapters": [
            [bytes.fromhex(k), True, None, timestamp] for k in fields["completed"]
        ],
        "series": [
            [bytes.fromhex(k), v, timestamp] for k, v in fields["series_status"].items()
        ],
        "lists": [],
        "members": [],
    }
    for entry in preview["public_lists_for_review"]:
        name = entry["name"]
        if (
            not isinstance(name, str)
            or not name
            or len(name) > 128
            or len(name.encode()) > 512
            # Unicode Cc controls: match the browser/Rust contract without
            # stripping or rejecting valid format characters such as ZWJ.
            or any(ord(c) < 32 or 0x7F <= ord(c) <= 0x9F for c in name)
        ):
            raise ValueError("invalid public list name")
        key = uuid.UUID(entry["source_id"]).bytes
        rows["lists"].append([key, name, timestamp])
        rows["members"].extend(
            [key, bytes.fromhex(series), pos, timestamp]
            for pos, series in enumerate(entry["series"])
        )
    if sum(map(len, rows.values())) > 2_000_000:
        raise ValueError("library record budget")
    for family, records in rows.items():
        records.sort(key=lambda row: record_key(family, row))
        if len({record_key(family, row) for row in records}) != len(records):
            raise ValueError("duplicate library record key")
    return rows


def local_records(rows):
    """Convert normalized tuples directly into the browser's public record schema."""
    return {
        "chapters": [
            dict(id=r[0].hex(), completed=r[1], page=r[2], updated_at=r[3])
            for r in rows["chapters"]
        ],
        "series": [
            dict(id=r[0].hex(), status=r[1], updated_at=r[2]) for r in rows["series"]
        ],
        "lists": [
            dict(id=r[0].hex(), name=r[1], updated_at=r[2]) for r in rows["lists"]
        ],
        "members": [
            dict(list=r[0].hex(), series=r[1].hex(), position=r[2], updated_at=r[3])
            for r in rows["members"]
        ],
    }
