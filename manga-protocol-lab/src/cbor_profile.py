"""Experimental RFC 8949 core-deterministic subset. Schema profile not yet approved."""


class InvalidCBOR(ValueError):
    pass


def head(major, n):
    if n < 0:
        raise InvalidCBOR("negative length")
    if n < 24:
        return bytes([(major << 5) | n])
    for ai, width in [(24, 1), (25, 2), (26, 4), (27, 8)]:
        if n < 1 << (width * 8):
            return bytes([(major << 5) | ai]) + n.to_bytes(width, "big")
    raise InvalidCBOR("integer outside uint64 representation")


def encode(x):
    if x is None:
        return b"\xf6"
    if type(x) is bool:
        return b"\xf5" if x else b"\xf4"
    if type(x) is int:
        return head(0, x) if x >= 0 else head(1, -1 - x)
    if type(x) is bytes:
        return head(2, len(x)) + x
    if type(x) is str:
        b = x.encode("utf-8")
        return head(3, len(b)) + b
    if type(x) is list:
        return head(4, len(x)) + b"".join(encode(i) for i in x)
    if type(x) is dict:
        pairs = sorted((encode(k), encode(v)) for k, v in x.items())
        return head(5, len(pairs)) + b"".join(k + v for k, v in pairs)
    raise InvalidCBOR("unsupported type")


def decode(data, max_bytes=1_000_000, max_depth=20, max_items=10_000):
    if len(data) > max_bytes:
        raise InvalidCBOR("byte budget")
    pos = 0
    items = 0

    def take(n):
        nonlocal pos
        if n > len(data) - pos:
            raise InvalidCBOR("truncated")
        b = data[pos : pos + n]
        pos += n
        return b

    def parse(depth):
        nonlocal pos, items
        if depth > max_depth:
            raise InvalidCBOR("depth budget")
        items += 1
        if items > max_items:
            raise InvalidCBOR("item budget")
        first = take(1)[0]
        major = first >> 5
        ai = first & 31
        if major == 7:
            if ai == 20:
                return False
            if ai == 21:
                return True
            if ai == 22:
                return None
            raise InvalidCBOR("float/simple value excluded by experiment profile")
        if ai < 24:
            n = ai
        elif ai in (24, 25, 26, 27):
            width = {24: 1, 25: 2, 26: 4, 27: 8}[ai]
            n = int.from_bytes(take(width), "big")
            if n < {24: 24, 25: 256, 26: 65536, 27: 4294967296}[ai]:
                raise InvalidCBOR("nonminimal integer/length")
        else:
            raise InvalidCBOR("indefinite/reserved encoding")
        if major == 0:
            return n
        if major == 1:
            return -1 - n
        if major == 2:
            return take(n)
        if major == 3:
            try:
                return take(n).decode("utf-8")
            except UnicodeDecodeError as e:
                raise InvalidCBOR("utf8") from e
        if major in (4, 5) and n > max_items - items:
            raise InvalidCBOR("declared collection budget")
        if major == 4:
            return [parse(depth + 1) for _ in range(n)]
        if major == 5:
            result = {}
            previous = None
            for _ in range(n):
                start = pos
                key = parse(depth + 1)
                key_bytes = data[start:pos]
                if previous is not None and key_bytes <= previous:
                    raise InvalidCBOR("map order or duplicate key")
                previous = key_bytes
                if type(key) not in (int, str, bytes):
                    raise InvalidCBOR("map key type")
                if key in result:
                    raise InvalidCBOR("duplicate semantic key")
                result[key] = parse(depth + 1)
            return result
        raise InvalidCBOR("tag/type excluded")

    value = parse(0)
    if pos != len(data):
        raise InvalidCBOR("trailing bytes")
    return value
