"""Reviewed MangaDex-shaped import plans for the PostgreSQL node.

Planning is offline and requires only a public key. Applying uses an existing
signing key, a pinned service key, local originals and explicit public-history
consent. No MangaDex login, scraping, downloads or inferred identity mappings.
"""

from __future__ import annotations
import argparse, base64, hashlib, ipaddress, json, os, secrets, time, uuid, urllib.request, urllib.error, urllib.parse
from pathlib import Path
from src.operator_json import read_json, save_json
from src.library_records import library_records, record_key
import cbor2
from coincurve import PrivateKey, PublicKeyXOnly
from src.mangadex_migration import (
    MigrationError,
    series_packet,
    chapter_packet,
    library_preview,
    id_map,
    source_id,
    target_id,
    original,
)


def dump(value):
    return json.dumps(
        value, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def b64(value):
    return base64.b64encode(value).decode()


def unsigned(public, kind, manifest, at):
    content = dump({"manifest": digest(manifest), "version": 1}).decode()
    event = dict(pubkey=public, created_at=at, kind=kind, tags=[], content=content)
    event["id"] = digest(dump([0, public, at, kind, [], content]))
    return event


def library_packet(preview, at):
    rows = library_records(preview, at)
    families = []
    objects = []
    total_buckets = 0
    total_bytes = 0
    for family, records in rows.items():
        key = lambda r: record_key(family, r)
        count = 1
        while True:
            buckets = [[] for _ in range(count)]
            for row in records:
                buckets[
                    int.from_bytes(hashlib.sha256(key(row)).digest()[:4], "big")
                    & (count - 1)
                ].append(row)
            encoded = [
                (
                    cbor2.dumps(
                        dict(type="library-chunk", version=1, family=family, entries=b),
                        canonical=True,
                    )
                    if b
                    else None
                )
                for b in buckets
            ]
            if all(
                len(b) <= 4096 and (blob is None or len(blob) <= 1_000_000)
                for b, blob in zip(buckets, encoded)
            ):
                break
            count *= 2
            if count > 2048:
                raise MigrationError("library bucket budget")
        desc = []
        for bucket, blob in zip(buckets, encoded):
            if blob is None:
                desc.append(None)
            else:
                desc.append([hashlib.sha256(blob).digest(), len(blob), len(bucket)])
                objects.append(dict(type="chunk", hash=digest(blob), data=b64(blob)))
                total_bytes += len(blob)
        families.append(dict(family=family, chunks=desc))
        total_buckets += count
    if total_buckets > 4096 or total_bytes > 256 * 1024 * 1024:
        raise MigrationError("library aggregate budget")
    root = cbor2.dumps(
        dict(type="library-root", version=1, families=families), canonical=True
    )
    if len(root) > 256 * 1024:
        raise MigrationError("library root budget")
    objects.append(dict(type="root", hash=digest(root), data=b64(root)))
    return root, objects


def prepare(bundle, originals, publisher, created_at, mapping=None):
    target_id(publisher)
    if type(created_at) is not int or not 0 <= created_at < 2**63:
        raise MigrationError("import timestamp bounds")
    mapping = mapping or {}
    maps = {k: id_map(mapping.get(k, {})) for k in ("series", "chapters")}
    if not isinstance(bundle, dict) or any(
        k not in ("series", "chapters", "library", "covers") for k in bundle
    ):
        raise MigrationError("bundle fields must be series, chapters, library, covers")
    if len(bundle.get("series", [])) + len(bundle.get("chapters", [])) > 10000:
        raise MigrationError("use multiple reviewed import batches")
    covers = bundle.get("covers", {})
    if not isinstance(covers, dict):
        raise MigrationError(
            "covers must map source series UUID to local original filename"
        )
    covers = {source_id(k): v for k, v in covers.items()}
    entries = []
    errors = []
    seen = set()
    review = []
    for resource in bundle.get("series", []):
        try:
            old = source_id(
                resource["id"] if "id" in resource else resource["data"]["id"]
            )
            packet = series_packet(resource, covers.get(old), originals)
            if ("series", old) in seen:
                raise MigrationError("duplicate source series")
            seen.add(("series", old))
            event = unsigned(publisher, 1002, packet["manifest"], created_at)
            if old in maps["series"] and maps["series"][old] != event["id"]:
                raise MigrationError("ambiguous existing series mapping")
            maps["series"][old] = event["id"]
            objects = [
                dict(hash=p["sha256"], path=p["name"], size=p["size"])
                for p in packet["local_original_inventory"]
            ]
            objects.append(
                dict(hash=packet["manifest_hash"], data=b64(packet["manifest"]))
            )
            entries.append(dict(source_id=old, event=event, objects=objects))
            review.append(
                {k: v for k, v in packet.items() if k not in ("metadata", "manifest")}
            )
        except (MigrationError, ValueError, KeyError, TypeError) as e:
            errors.append(dict(kind="series", error=str(e)))
    for old in covers:
        if ("series", old) not in seen:
            errors.append(
                dict(
                    kind="cover",
                    source_id=old,
                    error="cover needs a corresponding series resource",
                )
            )
    for item in bundle.get("chapters", []):
        try:
            packet = chapter_packet(
                item["chapter"], maps["series"], originals, item["ordered_originals"]
            )
            old = source_id(packet["local_source_record"]["id"])
            if ("chapters", old) in seen:
                raise MigrationError("duplicate source chapter")
            seen.add(("chapters", old))
            event = unsigned(publisher, 1001, packet["manifest"], created_at)
            if old in maps["chapters"] and maps["chapters"][old] != event["id"]:
                raise MigrationError("ambiguous existing chapter mapping")
            maps["chapters"][old] = event["id"]
            objects = [
                dict(hash=p["sha256"], path=p["name"], size=p["size"])
                for p in packet["local_original_inventory"]
            ]
            objects.append(
                dict(hash=packet["manifest_hash"], data=b64(packet["manifest"]))
            )
            entries.append(dict(source_id=old, event=event, objects=objects))
            review.append(
                {k: v for k, v in packet.items() if k not in ("metadata", "manifest")}
            )
        except (MigrationError, ValueError, KeyError, TypeError) as e:
            errors.append(dict(kind="chapter", error=str(e)))
    library = None
    if "library" in bundle:
        try:
            value = bundle["library"]
            preview = library_preview(
                value,
                maps["series"],
                maps["chapters"],
            )
            blockers = preview["unmapped"] + [
                dict(kind="list_member", source_id=x)
                for entry in preview["public_lists_for_review"]
                for x in entry["unmapped_series"]
            ]
            root, objects = library_packet(preview, created_at)
            library = dict(
                event=unsigned(publisher, 11999, root, created_at),
                objects=objects,
                unmapped=blockers,
                private_lists_excluded=len(
                    preview["private_lists_excluded_pending_review"]
                ),
                public_list_count=len(preview["public_lists_for_review"]),
            )
        except (MigrationError, ValueError, KeyError, TypeError) as e:
            errors.append(dict(kind="library", error=str(e)))
    plan = dict(
        version=1,
        publisher=publisher,
        created_at=created_at,
        entries=entries,
        library=library,
        mapping=maps,
        review=review,
        errors=errors,
        warnings=[
            "New events are authored by the selected importing key; no source author/group key is inferred.",
            "Local source UUID mappings are not protocol revision/equivalence links.",
            "Original order and bytes are preserved; changed files invalidate the plan.",
            "Library history becomes public only with explicit apply consent. Private legacy lists are excluded.",
            "Imported record update timestamps indicate import time, not invented reading dates.",
        ],
    )
    plan["plan_id"] = digest(dump(plan))
    return plan


class Signer:
    def __init__(self, secret):
        self.key = PrivateKey(bytes.fromhex(secret.strip()))
        self.public = PublicKeyXOnly.from_secret(self.key.secret).format().hex()

    def event(self, kind, tags, content, at=None):
        at = int(time.time()) if at is None else at
        message = dump([0, self.public, at, kind, tags, content])
        h = hashlib.sha256(message).digest()
        return dict(
            id=h.hex(),
            pubkey=self.public,
            created_at=at,
            kind=kind,
            tags=tags,
            content=content,
            sig=self.key.sign_schnorr(h).hex(),
        )

    def planned(self, e):
        result = self.event(e["kind"], e["tags"], e["content"], e["created_at"])
        if result["id"] != e["id"] or result["pubkey"] != e["pubkey"]:
            raise MigrationError("plan does not belong to signing key")
        return result


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


class Client:
    def __init__(self, origin, signer):
        p = urllib.parse.urlsplit(origin)
        if (
            p.username
            or p.password
            or p.query
            or p.fragment
            or p.path not in ("", "/")
            or not p.hostname
        ):
            raise MigrationError("origin required without credentials/path/query")
        local = False
        try:
            local = ipaddress.ip_address(p.hostname).is_loopback
        except ValueError:
            pass
        if p.scheme != "https" and not (p.scheme == "http" and local):
            raise MigrationError("HTTPS required except literal loopback")
        self.origin = origin.rstrip("/")
        self.signer = signer
        self.service_key = None
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}), NoRedirect()
        )

    def verify_response(self, response, url, method, authorization, raw):
        if self.service_key is None:
            raise MigrationError("service key must be pinned before requests")
        header = response.headers.get("X-Manga-Response-Proof", "")
        try:
            if len(header) > 16384 or not header.startswith("Nostr "):
                raise ValueError("missing proof")
            proof = json.loads(base64.b64decode(header[6:], validate=True))
            message = dump(
                [
                    0,
                    proof["pubkey"],
                    proof["created_at"],
                    proof["kind"],
                    proof["tags"],
                    proof["content"],
                ]
            )
            h = hashlib.sha256(message).digest()
            if (
                proof["kind"] != 20006
                or proof["pubkey"] != self.service_key
                or proof["id"] != h.hex()
                or abs(proof["created_at"] - time.time()) > 60
                or not PublicKeyXOnly(bytes.fromhex(self.service_key)).verify(
                    bytes.fromhex(proof["sig"]), h
                )
            ):
                raise ValueError("identity or signature")
            tags = {}
            for tag in proof["tags"]:
                if len(tag) >= 2:
                    tags.setdefault(tag[0], []).append(tag[1])
            for key, value in dict(
                protocol="manga-federation-response-v1",
                u=url,
                method=method,
                request=authorization["id"],
                status=str(response.status),
                payload=digest(raw),
            ).items():
                if tags.get(key) != [value]:
                    raise ValueError("response binding")
        except (ValueError, TypeError, KeyError) as error:
            raise MigrationError("service response proof rejected") from error

    def request(self, method, path, value=None, blob=None, missing_ok=False):
        if self.service_key is None:
            raise MigrationError("pin a service key before sending requests")
        body = b"" if value is None else dump(value)
        url = self.origin + path
        headers = {"Content-Type": "application/json"}
        if blob is not None:
            body = blob
            hash = digest(body)
            auth = self.signer.event(
                24242,
                [
                    ["t", "upload"],
                    ["server", self.origin],
                    ["x", hash],
                    ["expiration", str(int(time.time()) + 120)],
                    ["nonce", secrets.token_hex(16)],
                ],
                "Upload reviewed import original",
                int(time.time()) - 1,
            )
            headers["Content-Type"] = "application/octet-stream"
        else:
            auth = self.signer.event(
                27235,
                [
                    ["u", url],
                    ["method", method],
                    ["payload", digest(body)],
                    ["nonce", secrets.token_hex(16)],
                ],
                "",
            )
        headers["Authorization"] = "Nostr " + b64(dump(auth))
        req = urllib.request.Request(
            url, data=body if method != "GET" else None, headers=headers, method=method
        )
        try:
            with self.opener.open(req, timeout=20) as response:
                raw = response.read(2 * 1024 * 1024 + 1)
                if len(raw) > 2 * 1024 * 1024:
                    raise MigrationError("response byte budget")
                self.verify_response(response, url, method, auth, raw)
                if response.status not in (200, 202):
                    raise MigrationError("unexpected response status")
                return json.loads(raw)
        except urllib.error.HTTPError as e:
            raw = e.read(2 * 1024 * 1024 + 1)
            if len(raw) > 2 * 1024 * 1024:
                raise MigrationError("error response byte budget")
            self.verify_response(e, url, method, auth, raw)
            if missing_ok and method == "GET" and e.code == 404:
                return None
            raise MigrationError(
                f"node rejected {method} {path}: HTTP {e.code}"
            ) from None

    def verify_service(self, expected):
        target_id(expected)
        self.service_key = expected
        nonce = secrets.token_hex(32)
        event = self.request("GET", "/identity?nonce=" + nonce)
        message = dump(
            [
                0,
                event["pubkey"],
                event["created_at"],
                event["kind"],
                event["tags"],
                event["content"],
            ]
        )
        h = hashlib.sha256(message).digest()
        tags = {}
        for t in event["tags"]:
            if len(t) >= 2:
                tags.setdefault(t[0], []).append(t[1])
        if (
            event["pubkey"] != expected
            or event["kind"] != 20005
            or event["id"] != h.hex()
            or abs(event["created_at"] - time.time()) > 60
            or tags.get("nonce") != [nonce]
            or tags.get("endpoint") != [self.origin]
            or not PublicKeyXOnly(bytes.fromhex(expected)).verify(
                bytes.fromhex(event["sig"]), h
            )
        ):
            raise MigrationError("service identity verification failed")

    def projected(self, event_id):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            result = self.request("GET", "/events/" + event_id)
            if result.get("sig") and result.get("id") == event_id:
                return
            time.sleep(0.05)
        raise MigrationError("projection pending; safely retry the same plan")


def object_bytes(item, originals):
    if "data" in item:
        raw = base64.b64decode(item["data"], validate=True)
    else:
        checked = original(originals, item["path"])
        if checked["sha256"].hex() != item["hash"] or checked["size"] != item["size"]:
            raise MigrationError("original changed since review")
        root = Path(originals).resolve(strict=True)
        path = (root / item["path"]).resolve(strict=True)
        if not path.is_relative_to(root):
            raise MigrationError("original escapes selected directory")
        with path.open("rb") as f:
            raw = f.read(16 * 1024 * 1024 + 1)
    if len(raw) > 16 * 1024 * 1024:
        raise MigrationError("object byte budget")
    if digest(raw) != item["hash"]:
        raise MigrationError("plan object hash mismatch")
    return raw


def apply(
    plan,
    originals,
    origin,
    secret,
    service_key,
    journal,
    publish_public_library=False,
    accept_review=False,
):
    if not accept_review:
        raise MigrationError("explicit review acceptance required")
    payload = {k: v for k, v in plan.items() if k != "plan_id"}
    if (
        plan.get("version") != 1
        or digest(dump(payload)) != plan.get("plan_id")
        or plan.get("errors")
    ):
        raise MigrationError("invalid plan or unresolved plan errors")
    signer = Signer(secret)
    if signer.public != plan["publisher"]:
        raise MigrationError("signing key differs from reviewed publisher")
    if publish_public_library and plan.get("library") and plan["library"]["unmapped"]:
        raise MigrationError("unmapped library references require review")
    # Validate every selected byte and unsigned event before the first mutation.
    for entry in plan["entries"]:
        signer.planned(entry["event"])
        for item in entry["objects"]:
            object_bytes(item, originals)
    if publish_public_library and plan.get("library"):
        signer.planned(plan["library"]["event"])
        for item in plan["library"]["objects"]:
            object_bytes(item, originals)
    client = Client(origin, signer)
    client.verify_service(service_key)
    if publish_public_library and plan.get("library"):
        current = client.request("GET", "/libraries/" + signer.public, missing_ok=True)
        if (
            current is not None
            and current.get("event", {}).get("id") != plan["library"]["event"]["id"]
        ):
            raise MigrationError(
                "destination library exists; explicit merge or separate import is required"
            )
    journal = Path(journal)
    if journal.exists():
        state = read_json(journal)
        if (
            state.get("plan_id") != plan["plan_id"]
            or state.get("origin") != client.origin
            or state.get("service_key") != service_key
        ):
            raise MigrationError("journal belongs to another plan or destination")
    else:
        state = dict(
            plan_id=plan["plan_id"],
            origin=client.origin,
            service_key=service_key,
            completed=[],
            library_published=False,
        )
    save_json(journal, state)
    for entry in plan["entries"]:
        signed = signer.planned(entry["event"])
        # Replaying exact IDs also checks local tombstones rather than trusting a
        # possibly stale completion journal after restoring an older node backup.
        for item in entry["objects"]:
            client.request(
                "PUT", "/upload/" + item["hash"], blob=object_bytes(item, originals)
            )
        client.request("POST", "/publications", signed)
        client.projected(signed["id"])
        if signed["id"] not in state["completed"]:
            state["completed"].append(signed["id"])
        save_json(journal, state)
    library = plan.get("library")
    if publish_public_library and library:
        # Library staging uses NIP98 binding on the exact CBOR body, not Blossom.
        for item in library["objects"]:
            body = object_bytes(item, originals)
            url = client.origin + "/library/objects/" + item["type"]
            token = signer.event(
                27235,
                [
                    ["u", url],
                    ["method", "POST"],
                    ["payload", digest(body)],
                    ["nonce", secrets.token_hex(16)],
                ],
                "",
            )
            request = urllib.request.Request(
                url,
                data=body,
                headers={
                    "Authorization": "Nostr " + b64(dump(token)),
                    "Content-Type": "application/cbor",
                },
                method="POST",
            )
            try:
                with client.opener.open(request, timeout=20) as response:
                    if response.status != 200:
                        raise MigrationError("library staging not accepted")
                    raw = response.read(65537)
                    if len(raw) > 65536:
                        raise MigrationError("library staging response budget")
                    client.verify_response(response, url, "POST", token, raw)
            except urllib.error.HTTPError as e:
                raise MigrationError(
                    f"library staging rejected: HTTP {e.code}"
                ) from None
        signed = signer.planned(library["event"])
        client.request("POST", "/libraries", dict(event=signed, expected_head=None))
        client.projected(signed["id"])
        state["library_published"] = True
        save_json(journal, state)
    return dict(
        completed=len(state["completed"]),
        library_published=state["library_published"],
        mapping=plan["mapping"],
        journal=str(journal),
    )


def register_aliases(mapping, origin, secret, service_key, accept_review=False):
    if not accept_review:
        raise MigrationError("explicit alias review acceptance required")
    if not isinstance(mapping, dict) or set(mapping) - {"series", "chapters"}:
        raise MigrationError("series/chapters mapping required")
    items = []
    for source_kind, kind in [("series", "manga"), ("chapters", "chapter")]:
        values = id_map(mapping.get(source_kind, {}))
        if len(values) > 1000:
            raise MigrationError("alias batch budget; split reviewed mappings")
        items.extend(
            dict(kind=kind, id=k, target=v, preferred=True)
            for k, v in sorted(values.items())
        )
    client = Client(origin, Signer(secret))
    client.verify_service(service_key)
    for item in items:
        client.request("POST", "/admin/compatibility/aliases", item)
    return dict(
        aliases_registered=len(items),
        signed_publications_changed=False,
        retry="same mapping is idempotent; conflicting targets are rejected",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("plan")
    p.add_argument("bundle")
    p.add_argument("output")
    p.add_argument("--originals", required=True)
    p.add_argument("--publisher", required=True)
    p.add_argument("--timestamp", type=int, default=int(time.time()))
    p.add_argument("--mapping")
    p = sub.add_parser("apply")
    p.add_argument("plan")
    p.add_argument("--originals", required=True)
    p.add_argument("--origin", required=True)
    p.add_argument("--service-key", required=True)
    p.add_argument("--key-file", required=True)
    p.add_argument("--journal", required=True)
    p.add_argument("--accept-review", action="store_true")
    p.add_argument("--publish-public-library", action="store_true")
    p = sub.add_parser("aliases")
    p.add_argument("mapping")
    p.add_argument("--origin", required=True)
    p.add_argument("--service-key", required=True)
    p.add_argument("--key-file", required=True)
    p.add_argument("--accept-review", action="store_true")
    a = parser.parse_args()
    if a.command == "plan":
        result = prepare(
            read_json(a.bundle),
            a.originals,
            a.publisher,
            a.timestamp,
            read_json(a.mapping) if a.mapping else None,
        )
        save_json(a.output, result, exclusive=True)
        print(
            json.dumps(
                dict(
                    plan_id=result["plan_id"],
                    publications=len(result["entries"]),
                    errors=result["errors"],
                    library_included=result["library"] is not None,
                )
            )
        )
    else:
        key = Path(a.key_file)
        if key.stat().st_mode & 0o077 or key.stat().st_size > 128:
            raise MigrationError("signing key file must be private and bounded")
        if a.command == "aliases":
            print(
                json.dumps(
                    register_aliases(
                        read_json(a.mapping),
                        a.origin,
                        key.read_text(),
                        a.service_key,
                        a.accept_review,
                    )
                )
            )
        else:
            print(
                json.dumps(
                    apply(
                        read_json(a.plan),
                        a.originals,
                        a.origin,
                        key.read_text(),
                        a.service_key,
                        a.journal,
                        a.publish_public_library,
                        a.accept_review,
                    )
                )
            )


if __name__ == "__main__":
    main()
