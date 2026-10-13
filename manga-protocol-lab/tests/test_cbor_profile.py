import random
import unittest
import cbor2
from src.cbor_profile import encode, decode, InvalidCBOR


class CBORTests(unittest.TestCase):
    def test_known_vectors(self):
        for value, h in [
            (0, "00"),
            (23, "17"),
            (24, "1818"),
            (1000, "1903e8"),
            (-1, "20"),
            (b"ab", "426162"),
            ("hi", "626869"),
            ([1, 2, 3], "83010203"),
            (None, "f6"),
        ]:
            self.assertEqual(encode(value).hex(), h)
            self.assertEqual(decode(bytes.fromhex(h)), value)

    def test_core_order_differs_from_length_first_profile(self):
        data = {100: "a", -1: "b"}
        ours = encode(data)
        other = cbor2.dumps(data, canonical=True)
        self.assertEqual(ours.hex(), "a218646161206162")
        self.assertNotEqual(ours, other)
        self.assertEqual(cbor2.loads(ours), cbor2.loads(other))
        with self.assertRaises(InvalidCBOR):
            decode(other)

    def test_positive_integer_schema_keys_agree_with_cbor2(self):
        rng = random.Random(47)
        for _ in range(100):
            value = {
                k: bytes([rng.randrange(256)]) * rng.randrange(12)
                for k in rng.sample(range(1000), 10)
            }
            self.assertEqual(encode(value), cbor2.dumps(value, canonical=True))
            self.assertEqual(decode(encode(value)), value)

    def test_duplicate_key_rejected(self):
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("a201020103"))

    def test_nonminimal_and_truncated_lengths_rejected(self):
        payload = b"a" * 16
        self.assertEqual(decode(b"\x50" + payload), payload)
        with self.assertRaises(InvalidCBOR):
            decode(b"\x58\x10" + payload)
        with self.assertRaises(InvalidCBOR):
            decode(b"\x50abc")

    def test_nonminimal_integer_rejected(self):
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("1801"))

    def test_indefinite_array_rejected(self):
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("9f0102ff"))

    def test_trailing_object_rejected(self):
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("0102"))

    def test_invalid_utf8_rejected(self):
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("61ff"))

    def test_depth_budget(self):
        with self.assertRaises(InvalidCBOR):
            decode(b"\x81" * 30 + b"\x00", max_depth=10)

    def test_collection_budget(self):
        # A complete, otherwise valid collection distinguishes budget rejection
        # from the unrelated truncation error in the hostile length-only case.
        collection = b"\x8b" + b"\x00" * 11
        self.assertEqual(decode(collection, max_items=12), [0] * 11)
        with self.assertRaises(InvalidCBOR):
            decode(collection, max_items=11)
        with self.assertRaises(InvalidCBOR):
            decode(bytes.fromhex("9bffffffffffffffff"))

    def test_unknown_optional_field_bytes_preserved(self):
        data = encode({1: "known", 9999: ["unknown", b"extension"]})
        decoded = decode(data)
        self.assertEqual(decoded[1], "known")
        self.assertEqual(encode(decoded), data)


if __name__ == "__main__":
    unittest.main()
