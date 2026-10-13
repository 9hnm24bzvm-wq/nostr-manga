import socket
from operator_support import OperatorCase
from test_migration_plan_cli import PUBLIC


class AliasCLI(OperatorCase):
    def test_alias_consent_and_all_inputs_validate_before_network(self):
        key = self.root / "key"
        key.write_text("01".zfill(64))
        key.chmod(0o600)
        for case, value in [
            ("consent", {}),
            ("bad_uuid", {"series": {"bad": "ab" * 32}}),
            ("bad_id", {"series": {"00000000-0000-4000-8000-000000000001": "bad"}}),
            ("unknown_field", {"unknown": {}}),
            ("key_permissions", {}),
        ]:
            mapping = self.write("mapping.json", value)
            if case == "key_permissions":
                key.chmod(0o644)
            with socket.socket() as witness:
                witness.bind(("127.0.0.1", 0))
                witness.listen()
                witness.settimeout(0.1)
                result = self.cli(
                    "src.postgres_migration",
                    "aliases",
                    mapping,
                    "--origin",
                    f"http://127.0.0.1:{witness.getsockname()[1]}",
                    "--service-key",
                    PUBLIC,
                    "--key-file",
                    key,
                    *([] if case == "consent" else ["--accept-review"]),
                    check=False,
                )
                self.assertNotEqual(result.returncode, 0, case)
                with self.assertRaises(TimeoutError):
                    witness.accept()
            key.chmod(0o600)
