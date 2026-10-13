"""Regenerate literal vectors using independent Python libraries, never Rust helpers.

Public synthetic signing key 1 is a fixture, never a deployment credential.
Requires the project's pinned cbor2/coincurve packages. Run from project root.
"""

import datetime
import hashlib
import json
from pathlib import Path

import cbor2
from coincurve import PrivateKey

OUT = Path(__file__).parent


def encoded(value):
    return cbor2.dumps(value, canonical=True).hex()


minimal = {"version": 1, "titles": {"en": "Test"}}
rich = {
    "version": 1,
    "titles": {"en": "Example"},
    "altTitles": [{"ja": "例"}],
    "categories": ["Drama", "drama"],
    "description": {"en": ""},
    "credits": [{"role": "artist", "name": "Name"}],
    "originalLanguage": "ja",
    "year": 2026,
    "status": "ongoing",
    "contentRating": "safe",
    "publicationDemographic": "shounen",
    "future": {"anything": True},
}
cover = {"sha256": "ab" * 32, "size": 10, "mime": "image/png", "width": 1, "height": 1}
expected = {
    "titles": {"en": "Example"},
    "altTitles": [{"ja": "例"}],
    "description": {"en": ""},
    "categories": ["Drama", "drama"],
    "credits": [{"role": "artist", "name": "Name"}],
    "originalLanguage": "ja",
    "status": "ongoing",
    "contentRating": "safe",
    "publicationDemographic": "shounen",
    "year": 2026,
    "cover": None,
}
series = [
    {"name": "rich descriptive fields", "hex": encoded(rich), "expected": expected}
]
for required, present, accepted in [
    (True, True, True),
    (False, True, False),
    (True, False, False),
    (False, False, True),
]:
    value = dict(minimal)
    if required:
        value["required"] = ["series-cover-v1"]
    if present:
        value["cover"] = cover
    series.append(
        {
            "name": f"cover marker={required} present={present}",
            "hex": encoded(value),
            "accepted": accepted,
            "cover": cover if present and accepted else None,
        }
    )
for name, value, accepted in [
    ("unsupported version", {**minimal, "version": 2}, False),
    ("empty titles", {**minimal, "titles": {}}, False),
    ("unknown required feature", {**minimal, "required": ["unknown"]}, False),
    ("unknown optional feature", {**minimal, "optional": "unknown"}, True),
    (
        "empty cover",
        {**minimal, "required": ["series-cover-v1"], "cover": {**cover, "size": 0}},
        False,
    ),
]:
    series.append({"name": name, "hex": encoded(value), "accepted": accepted})

chapters = []
for language in [
    "en",
    "en-US",
    "zh-Hant-TW",
    "i-klingon",
    "x-test",
    "",
    "-",
    "en_US",
    "en--US",
    "123",
]:
    value = {
        "version": 1,
        "series": "ab" * 32,
        "language": language,
        "chapter": "7.5",
        "volume": "02",
        "title": "例",
        "pages": [{**cover, "sha256": "cd" * 32}],
    }
    chapters.append(
        {
            "language": language,
            "hex": encoded(value),
            "accepted": language
            in ["en", "en-US", "zh-Hant-TW", "i-klingon", "x-test"],
        }
    )

unsigned = [
    0,
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    1700000000,
    1,
    [["t", "manga"]],
    "Hello, 漫画",
]
event_id = hashlib.sha256(
    json.dumps(unsigned, ensure_ascii=False, separators=(",", ":")).encode()
).digest()
event = dict(zip(["pubkey", "created_at", "kind", "tags", "content"], unsigned[1:]))
event.update(
    id=event_id.hex(),
    sig=PrivateKey((1).to_bytes(32, "big"))
    .sign_schnorr(event_id, aux_randomness=bytes(32))
    .hex(),
)

uuids = []
for kind, target in [("manga", "test"), ("chapter", "test"), ("manga", "漫画")]:
    value = bytearray(
        hashlib.sha256(f"manga-compat-v1:{kind}:{target}".encode()).digest()[:16]
    )
    value[6] = (value[6] & 15) | 128
    value[8] = (value[8] & 63) | 128
    import uuid

    uuids.append(
        {"kind": kind, "target": target, "uuid": str(uuid.UUID(bytes=bytes(value)))}
    )
dates = []
for text in [
    "1970-01-01T00:00:00",
    "2000-02-29T12:34:56",
    "2026-10-10T00:00:00",
    "9999-12-31T23:59:59",
]:
    date = datetime.datetime.fromisoformat(text).replace(tzinfo=datetime.timezone.utc)
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    dates.append(
        {"text": text, "seconds": (date - epoch).days * 86400 + (date - epoch).seconds}
    )

for name, value in [
    ("series.json", series),
    ("chapters.json", chapters),
    ("signed-event.json", event),
    ("compatibility.json", {"uuids": uuids, "dates": dates}),
]:
    (OUT / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
