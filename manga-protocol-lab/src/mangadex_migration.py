"""Offline, unsigned migration packets. No API calls, downloading, signing or publication.

Field compatibility follows the official MangaDex OpenAPI 5.13.1 snapshot examined
on 2026-10-06. Proposed target metadata fields remain experimental.
"""

from __future__ import annotations
import argparse, hashlib, io, json, os, re, uuid
from pathlib import Path
from PIL import Image
from src.cbor_profile import encode
from src.operator_json import read_json


class MigrationError(ValueError):
    pass


def source_id(value):
    try:
        return str(uuid.UUID(value))
    except (ValueError, TypeError, AttributeError):
        raise MigrationError("expected MangaDex UUID") from None


def target_id(value):
    if not isinstance(value, str) or re.fullmatch("[0-9a-f]{64}", value) is None:
        raise MigrationError("expected exact lowercase publication ID")
    return value


def resource(value, kind):
    if isinstance(value, dict) and "data" in value:
        value = value["data"]
    if (
        not isinstance(value, dict)
        or value.get("type") != kind
        or not isinstance(value.get("attributes"), dict)
    ):
        raise MigrationError("wrong resource shape")
    source_id(value.get("id"))
    return value


def id_map(mapping):
    out = {}
    if not isinstance(mapping, dict):
        raise MigrationError("mapping object required")
    for old, new in mapping.items():
        old = source_id(old)
        new = target_id(new)
        if old in out and out[old] != new:
            raise MigrationError("ambiguous source identity mapping")
        out[old] = new
    return out


def text_field(attrs, name):
    v = attrs.get(name)
    if v is not None and not isinstance(v, str):
        raise MigrationError(name + " must remain a string or null")
    return v


def credit_names(source, roles):
    credits = []
    unresolved = []
    for rel in source.get("relationships", []):
        role = roles.get(rel.get("type"))
        if role is None:
            continue
        name = (rel.get("attributes") or {}).get("name")
        if isinstance(name, str) and name.strip():
            credits.append({"role": role, "name": name})
        else:
            unresolved.append(
                {"type": rel["type"], "source_id": source_id(rel.get("id"))}
            )
    return credits, unresolved


def series_packet(value, cover_file=None, originals=None):
    source = resource(value, "manga")
    attrs = source["attributes"]
    titles = attrs.get("title")
    if (
        not isinstance(titles, dict)
        or not titles
        or any(
            not isinstance(k, str) or not isinstance(v, str) or not v.strip()
            for k, v in titles.items()
        )
    ):
        raise MigrationError("localized titles required")
    metadata = {"version": 1, "titles": titles}
    # Descriptive fields are optional experimental metadata, never identity/revision links.
    for key in [
        "altTitles",
        "description",
        "originalLanguage",
        "status",
        "year",
        "contentRating",
        "publicationDemographic",
    ]:
        if key in attrs and attrs[key] is not None:
            metadata[key] = attrs[key]
    categories = []
    for tag in attrs.get("tags", []):
        names = tag.get("attributes", {}).get("name", {})
        if isinstance(names, dict) and names:
            label = names.get("en") or names[sorted(names)[0]]
            if not isinstance(label, str):
                raise MigrationError("category label")
            if label not in categories:
                categories.append(label)
    if categories:
        metadata["categories"] = categories
    credits, unresolved = credit_names(source, {"author": "author", "artist": "artist"})
    if credits:
        metadata["credits"] = credits
    inventory = []
    if cover_file is not None:
        if originals is None:
            raise MigrationError("local originals directory required for cover")
        cover = original(originals, cover_file)
        metadata["cover"] = cover
        metadata["required"] = ["series-cover-v1"]
        inventory = [
            dict(
                name=cover_file,
                sha256=cover["sha256"].hex(),
                **{k: v for k, v in cover.items() if k != "sha256"},
            )
        ]
    data = encode(metadata)
    if len(data) > 1_000_000:
        raise MigrationError("manifest budget")
    return {
        "metadata": metadata,
        "manifest": data,
        "manifest_hash": hashlib.sha256(data).hexdigest(),
        "local_source_record": source,
        "local_original_inventory": inventory,
        "unresolved_credit_relationships": unresolved,
        "warnings": [
            "Source relationships and UUIDs remain in the local sidecar, not protocol identity links.",
            "A new publication must be signed by its actual importer or authorized group; no source author key is inferred.",
        ],
    }


def original(root, name, allowed=("image/png", "image/jpeg", "image/webp")):
    root = Path(root).resolve(strict=True)
    if (
        not isinstance(name, str)
        or not name
        or "\\" in name
        or Path(name).is_absolute()
        or ".." in Path(name).parts
    ):
        raise MigrationError("unsafe original path")
    try:
        path = (root / name).resolve(strict=True)
    except OSError:
        raise MigrationError("original missing") from None
    if not path.is_relative_to(root) or not path.is_file():
        raise MigrationError("original escapes selected directory")
    if path.stat().st_size > 16 * 1024 * 1024:
        raise MigrationError("original exceeds instance byte limit")
    raw = path.read_bytes()
    if len(raw) > 16 * 1024 * 1024:
        raise MigrationError("original byte limit")
    try:
        with Image.open(io.BytesIO(raw)) as im:
            mime = {"PNG": "image/png", "JPEG": "image/jpeg", "WEBP": "image/webp"}.get(
                im.format
            )
            if mime not in allowed:
                raise MigrationError("image format not allowed")
            w, h = im.size
            if w < 1 or h < 1 or w * h > 40_000_000:
                raise MigrationError("pixel budget")
            im.load()
    except MigrationError:
        raise
    except Exception as exc:
        raise MigrationError("original does not decode") from exc
    return {
        "sha256": hashlib.sha256(raw).digest(),
        "size": len(raw),
        "mime": mime,
        "width": w,
        "height": h,
    }


def chapter_packet(
    value,
    series_mapping,
    root,
    ordered_originals,
    allowed=("image/png", "image/jpeg", "image/webp"),
):
    source = resource(value, "chapter")
    attrs = source["attributes"]
    mapping = id_map(series_mapping)
    manga = [
        source_id(r.get("id"))
        for r in source.get("relationships", [])
        if r.get("type") == "manga"
    ]
    if len(manga) != 1 or manga[0] not in mapping:
        raise MigrationError("one explicitly mapped series relationship required")
    if attrs.get("externalUrl"):
        raise MigrationError(
            "external-only/link chapter requires manual review; no hotlink import"
        )
    if (
        not isinstance(ordered_originals, list)
        or not ordered_originals
        or len(ordered_originals) > 10000
    ):
        raise MigrationError("ordered local originals required")
    if type(attrs.get("pages")) is not int or attrs["pages"] != len(ordered_originals):
        raise MigrationError("original count differs from source chapter")
    language = attrs.get("translatedLanguage")
    if not isinstance(language, str) or not language:
        raise MigrationError("chapter language required")
    pages = [original(root, n, allowed) for n in ordered_originals]
    metadata = {
        "version": 1,
        "series": mapping[manga[0]],
        "language": language,
        "pages": pages,
    }
    for key in ["chapter", "volume", "title"]:
        value = text_field(attrs, key)
        if value is not None:
            metadata[key] = value
    credits, unresolved = credit_names(source, {"scanlation_group": "scanlation"})
    if credits:
        metadata["credits"] = credits
    data = encode(metadata)
    if len(data) > 1_000_000:
        raise MigrationError("manifest budget")
    return {
        "metadata": metadata,
        "manifest": data,
        "manifest_hash": hashlib.sha256(data).hexdigest(),
        "local_source_record": source,
        "unresolved_credit_relationships": unresolved,
        "local_original_inventory": [
            dict(
                name=name,
                sha256=page["sha256"].hex(),
                **{k: v for k, v in page.items() if k != "sha256"},
            )
            for name, page in zip(ordered_originals, pages)
        ],
        "warnings": [
            "Original bytes were inspected locally; no image URLs or data-saver images were fetched.",
            "Source uploader/group UUIDs are not Nostr keys. Attribution and publishing authority require review.",
        ],
    }


def library_preview(value, series_mapping, chapter_mapping):
    # This is our supplied-export envelope, not an individual MangaDex response.
    # Validate it before selecting fields so unsupported state is never discarded.
    if not isinstance(value, dict):
        raise MigrationError("library export object required")
    unsupported = set(value) - {"read_markers", "statuses", "lists"}
    if unsupported:
        raise MigrationError(
            "unsupported library fields: " + ", ".join(sorted(unsupported))
        )
    read_markers = value.get("read_markers", [])
    statuses = value.get("statuses", {})
    lists = value.get("lists", [])
    series_mapping = id_map(series_mapping)
    chapter_mapping = id_map(chapter_mapping)
    reads = (
        read_markers.get("data", []) if isinstance(read_markers, dict) else read_markers
    )
    if isinstance(reads, dict):
        reads = [item for values in reads.values() for item in values]
    if not isinstance(reads, list):
        raise MigrationError("read markers array or grouped arrays required")
    reads = list(dict.fromkeys(source_id(x) for x in reads))
    completed = sorted({chapter_mapping[x] for x in reads if x in chapter_mapping})
    unresolved = [
        {"kind": "chapter", "source_id": x} for x in reads if x not in chapter_mapping
    ]
    states = statuses.get("statuses", statuses)
    if not isinstance(states, dict):
        raise MigrationError("statuses object")
    proposed_statuses = {}
    for old, status in states.items():
        old = source_id(old)
        if status not in [
            "reading",
            "on_hold",
            "plan_to_read",
            "dropped",
            "re_reading",
            "completed",
        ]:
            raise MigrationError("unknown source reading status")
        if old not in series_mapping:
            unresolved.append({"kind": "series", "source_id": old})
        else:
            proposed_statuses[series_mapping[old]] = status
    public_lists = []
    private_lists = []
    for item in lists:
        item = resource(item, "custom_list")
        attrs = item["attributes"]
        visibility = attrs.get("visibility")
        if visibility not in ["public", "private"]:
            raise MigrationError("list visibility missing")
        selected = []
        missing = []
        for rel in item.get("relationships", []):
            if rel.get("type") != "manga":
                continue
            old = source_id(rel.get("id"))
            if old in series_mapping:
                selected.append(series_mapping[old])
            else:
                missing.append(old)
        preview = {
            "name": text_field(attrs, "name"),
            "series": list(dict.fromkeys(selected)),
            "unmapped_series": missing,
            "source_id": source_id(item["id"]),
        }
        if visibility == "private":
            private_lists.append(preview)
        else:
            public_lists.append(preview)
    fields = {
        "completed": completed,
        "series_status": proposed_statuses,
        "lists": [{"name": v["name"], "series": v["series"]} for v in public_lists],
    }
    content = json.dumps(fields, separators=(",", ":"), ensure_ascii=False)
    example_envelope = {
        "id": "0" * 64,
        "pubkey": "0" * 64,
        "created_at": 1999999999,
        "kind": 11999,
        "tags": [],
        "content": content,
        "sig": "0" * 128,
    }
    estimated = len(
        json.dumps(example_envelope, separators=(",", ":"), ensure_ascii=False).encode()
    )
    return {
        "unsigned_preview_only": True,
        "requires_publication_consent": True,
        "experimental_library_fields": fields,
        "illustrative_full_event_bytes": estimated,
        "exceeds_default_event_budget": estimated > 1_000_000,
        "public_lists_for_review": public_lists,
        "private_lists_excluded_pending_review": private_lists,
        "unmapped": unresolved,
        "warnings": [
            "The target library is public. Do not publish private legacy history or lists automatically.",
            "Only explicit exact-ID mappings are used; matching by chapter number/title is prohibited.",
            "A series status of completed does not fabricate chapter read markers.",
            "Status/list field names are migration proposals, not finalized protocol schema.",
        ],
    }


def read_json_file(path):
    try:
        return read_json(path, 16 * 1024 * 1024)
    except ValueError as error:
        raise MigrationError(str(error)) from error


def write_private(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as file:
        file.write(data)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("kind", choices=["series", "chapter", "library"])
    p.add_argument("input")
    p.add_argument("output")
    p.add_argument("--mapping")
    p.add_argument("--originals")
    p.add_argument("--cover")
    args = p.parse_args()
    value = read_json_file(args.input)
    mapping = read_json_file(args.mapping) if args.mapping else {}
    if args.kind == "series":
        result = series_packet(value, args.cover, args.originals)
    elif args.kind == "chapter":
        if not args.originals:
            raise MigrationError("--originals required")
        result = chapter_packet(
            value["chapter"],
            mapping.get("series", {}),
            args.originals,
            value["ordered_originals"],
        )
    else:
        result = library_preview(
            value,
            mapping.get("series", {}),
            mapping.get("chapters", {}),
        )
    out = Path(args.output)
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    if "manifest" in result:
        write_private(out / "manifest.cbor", result.pop("manifest"))
        result.pop(
            "metadata"
        )  # Bytes-valued page hashes remain in the actual CBOR file.
    write_private(
        out / "review.json", json.dumps(result, indent=2, ensure_ascii=False).encode()
    )
    print(str(out))


if __name__ == "__main__":
    main()
