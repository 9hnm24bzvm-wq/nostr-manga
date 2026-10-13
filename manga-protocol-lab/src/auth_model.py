"""Nostr/BUD auth validation experiment. Not an audited production library."""

import base64
import hashlib
import json
from coincurve import PrivateKey, PublicKeyXOnly


class Invalid(Exception):
    pass


def event_digest(e):
    raw = json.dumps(
        [0, e["pubkey"], e["created_at"], e["kind"], e["tags"], e["content"]],
        ensure_ascii=False,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()
    return hashlib.sha256(raw).digest()


def sign_event(secret, kind, created_at, tags, content=""):
    key = PrivateKey(secret)
    e = dict(
        pubkey=key.public_key_xonly.format().hex(),
        kind=kind,
        created_at=created_at,
        tags=tags,
        content=content,
    )
    h = event_digest(e)
    e.update(id=h.hex(), sig=key.sign_schnorr(h).hex())
    return e


def validate_signature(e):
    try:
        if type(e["created_at"]) is not int or type(e["kind"]) is not int:
            raise Invalid("integer fields")
        if len(e["pubkey"]) != 64 or e["pubkey"] != e["pubkey"].lower():
            raise Invalid("pubkey encoding")
        if not isinstance(e["content"], str):
            raise Invalid("content type")
        if not isinstance(e["tags"], list) or any(
            not isinstance(t, list) or not t or any(not isinstance(x, str) for x in t)
            for t in e["tags"]
        ):
            raise Invalid("tag encoding")
        h = event_digest(e)
        if e["id"] != h.hex() or not PublicKeyXOnly(bytes.fromhex(e["pubkey"])).verify(
            bytes.fromhex(e["sig"]), h
        ):
            raise Invalid("signature or id")
    except (KeyError, TypeError, ValueError) as exc:
        raise Invalid("malformed event") from exc


def tags(e, name):
    return [t[1] for t in e["tags"] if t[0] == name and len(t) >= 2]


def one(e, name):
    values = tags(e, name)
    if len(values) != 1:
        raise Invalid("missing or duplicate " + name)
    return values[0]


class HttpAuth:
    def __init__(self, approved, window=60):
        self.approved = set(approved)
        self.window = window
        self.seen = {}

    def validate(self, e, url, method, body, now):
        validate_signature(e)
        if e["kind"] != 27235 or e["pubkey"] not in self.approved:
            raise Invalid("kind or peer approval")
        if abs(now - e["created_at"]) > self.window:
            raise Invalid("freshness")
        if one(e, "u") != url or one(e, "method") != method:
            raise Invalid("request binding")
        # Stricter project profile: bind even an empty mutating request body.
        if body or method in {"POST", "PUT", "PATCH", "DELETE"}:
            if one(e, "payload") != hashlib.sha256(body).hexdigest():
                raise Invalid("body binding")
        self.seen = {k: v for k, v in self.seen.items() if v >= now}
        if e["id"] in self.seen:
            raise Invalid("replay")
        self.seen[e["id"]] = e["created_at"] + self.window
        return e["pubkey"]


def validate_blob_token(
    e, server_domain, action, blob_hash, now, approved, max_ttl=300, max_hashes=100
):
    """300s/100 hashes are experiment settings, NOT accepted protocol constants."""
    validate_signature(e)
    if e["kind"] != 24242 or e["pubkey"] not in approved:
        raise Invalid("kind or approval")
    if not e["content"]:
        raise Invalid("missing intended-use explanation")
    try:
        expires = int(one(e, "expiration"))
    except ValueError as exc:
        raise Invalid("expiration") from exc
    if not e["created_at"] < now < expires or expires - e["created_at"] > max_ttl:
        raise Invalid("lifetime")
    if one(e, "t") != action:
        raise Invalid("action")
    servers = tags(e, "server")
    if (
        not servers
        or any(s != s.lower() or "://" in s or "/" in s for s in servers)
        or server_domain not in servers
    ):
        raise Invalid("server scope")
    hashes = tags(e, "x")
    if not 1 <= len(hashes) <= max_hashes or blob_hash not in hashes:
        raise Invalid("hash scope")
    for h in hashes:
        if len(h) != 64 or any(c not in "0123456789abcdef" for c in h):
            raise Invalid("hash encoding")
    return e["pubkey"]
