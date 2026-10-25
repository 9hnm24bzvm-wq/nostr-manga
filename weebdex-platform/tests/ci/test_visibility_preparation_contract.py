"""Source-only preparation contracts; not Rust/PostgreSQL or role acceptance.

Run with: python3 -m unittest discover -s tests/ci -p test_visibility_preparation_contract.py
No database, listener, compiler, credentials, or permission changes are used.
"""
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT.parent


class VisibilityPreparationContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.main = (ROOT / 'backend/src/main.rs').read_text()
        cls.facade = (ROOT / 'backend/src/protocol/mod.rs').read_text()
        cls.policy = (ROOT / 'backend/src/protocol/visibility.rs').read_text()
        cls.bridge = (SOURCE / 'manga-protocol-lab/server/src/platform_bridge.rs').read_text()

    def test_only_explicit_migrate_installs_before_return(self):
        preparation, runtime = self.main.split('state.db.verify_schema(domains::MIGRATIONS).await?;', 1)
        branch = preparation.split('if action == "migrate" {', 1)[1]
        self.assertLess(branch.index('state.db.migrate('), branch.index('install_visibility_function()'))
        self.assertLess(branch.index('install_visibility_function()'), branch.index('return Ok(())'))
        self.assertEqual(self.main.count('install_visibility_function()'), 1)
        self.assertNotIn('install_visibility_function', runtime)
        self.assertNotIn('.migrate(', runtime)

    def test_runtime_verifies_before_verify_return_or_any_runtime_owner(self):
        check = self.main.index('state.protocol.verify_visibility_function().await?;')
        for boundary in ('if action == "verify"', 'TcpListener::bind',
                         'configure_publication(', 'spawn_importer(',
                         'recover_storage(', 'spawn_storage_maintenance(',
                         'state.runtime.mark_ready()'):
            self.assertLess(check, self.main.index(boundary), boundary)
        self.assertLess(self.main.index('if action == "check-config"'),
                        self.main.index('AppState::connect('))

    def test_runtime_validation_is_read_only_consistent_and_fails_closed(self):
        method = self.policy.split('pub async fn verify_visibility_function', 1)[1].split('#[cfg(test)]', 1)[0]
        for needed in ('IsolationLevel::RepeatableRead', '.read_only(true)',
                       'verify_policy(&*tx, &expected).await?', 'if result.is_err()',
                       'self.schema_ready.store(false, Ordering::Release)'):
            self.assertIn(needed, method)
        for forbidden in ('.batch_execute(', '.execute(', 'install_visibility_function'):
            self.assertNotIn(forbidden, method)
        schema = self.facade.split('pub async fn verify_schema', 1)[1].split('pub fn is_ready', 1)[0]
        self.assertLess(schema.index('store(false'), schema.index('self.repository.verify_schema().await?'))

    def test_preparation_is_locked_transactional_and_attests_only_after_validation(self):
        method = self.policy.split('pub async fn install_visibility_function', 1)[1].split('pub async fn verify_visibility_function', 1)[0]
        ordered = ('client.transaction().await?', 'pg_advisory_xact_lock(697241114)',
                   'tx.batch_execute(&expected.sql).await?', 'verify_function(&*tx, &expected).await?',
                   'INSERT INTO platform.protocol_visibility_policy', 'tx.commit().await?')
        self.assertEqual(sorted(method.index(x) for x in ordered), [method.index(x) for x in ordered])
        self.assertIn('pg_advisory_xact_lock(697241114)', (ROOT / 'backend/src/db.rs').read_text())
        self.assertIn("statement_timeout = '60s'", method)
        self.assertIn("lock_timeout = '3s'", method)

    def test_installed_definition_is_not_blessed_by_attestation_alone(self):
        verification = self.policy.split('async fn verify_policy', 1)[1].split('impl ProtocolFacade', 1)[0]
        self.assertIn('SELECT version,mode,checksum FROM platform.protocol_visibility_policy WHERE singleton', verification)
        self.assertIn('attestation_matches(expected, actual)', verification)
        self.assertIn('verify_function(client, expected).await', verification)
        matcher = self.policy.split('fn attestation_matches', 1)[1].split('async fn verify_function', 1)[0]
        for needed in ('actual.is_some_and', 'version == POLICY_VERSION', 'mode == expected.mode',
                       'Sha256::digest(expected.sql.as_bytes())'):
            self.assertIn(needed, matcher)
        function = self.policy.split('fn policy_matches', 1)[1].split('fn attestation_matches', 1)[0]
        self.assertIn('actual.is_some_and', function)
        self.assertIn('body == expected.body && language == expected.language && attributes', function)

    def test_function_signature_security_and_execution_attributes_are_checked(self):
        query = self.policy.split('const INSTALLED_POLICY: &str = "', 1)[1].split('";', 1)[0]
        for needed in ("p.prokind = 'f'", "p.prorettype = 'pg_catalog.bool'::pg_catalog.regtype",
                       "p.proargnames = ARRAY['source_event']::pg_catalog.text[]",
                       'p.proargmodes IS NULL', 'p.proallargtypes IS NULL', 'p.pronargdefaults = 0',
                       'p.provariadic = 0', 'NOT p.proretset', 'NOT p.proisstrict',
                       'NOT p.prosecdef', 'NOT p.proleakproof', "p.provolatile = 's'",
                       "p.proparallel = 'u'", 'p.proconfig IS NULL', 'p.prosupport = 0',
                       'p.probin IS NULL', "pg_catalog.has_function_privilege(p.oid, 'EXECUTE')",
                       'pg_catalog.pg_proc', 'pg_catalog.pg_language', 'pg_catalog.to_regprocedure(',
                       "'platform.protocol_source_is_visible(pg_catalog.text)'", 'COALESCE(', 'FALSE)'):
            self.assertIn(needed, query)
        self.assertFalse(re.search(r'\b(?:CREATE|ALTER|DROP|INSERT|UPDATE|DELETE)\b', query))

    def test_canonical_export_preserves_exact_preexisting_generated_sql(self):
        # Pin the existing wrapper/body template, not a downstream policy copy.
        wrapper = self.bridge.split('pub fn visibility_function_sql()', 1)[1].split('pub fn visibility_function_body()', 1)[0]
        body = self.bridge.split('pub fn visibility_function_body()', 1)[1].split('fn deterministic_json', 1)[0]
        actual_wrapper = re.search(r'"(CREATE OR REPLACE FUNCTION[^"\n]+)"', wrapper)[1]
        actual_body = re.search(r'"( BEGIN IF[^"\n]+)"', body)[1]
        historical = "CREATE OR REPLACE FUNCTION platform.protocol_source_is_visible(source_event TEXT) RETURNS BOOLEAN LANGUAGE plpgsql STABLE AS $bridge$ BEGIN IF source_event IS NULL THEN RETURN TRUE; END IF; RETURN EXISTS(SELECT 1 FROM events e WHERE encode(e.id,'hex')=source_event AND e.kind IN(1001,1002) AND {VISIBLE}); EXCEPTION WHEN undefined_table OR undefined_column THEN RETURN FALSE; END; $bridge$;"
        self.assertEqual(actual_wrapper.replace('{}', actual_body), historical)
        self.assertIn('visibility_function_body()', wrapper)
        self.assertIn('manga_node::platform_bridge::visibility_function_body()', self.policy)
        self.assertIn('manga_node::platform_bridge::visibility_function_sql()', self.policy)
        self.assertNotIn('quarantined_blobs', self.policy)

    def test_standalone_policy_is_exact_null_only_and_mode_specific(self):
        self.assertIn('const STANDALONE_BODY: &str = " SELECT source_event IS NULL ";', self.policy)
        self.assertIn('mode: "native-only"', self.policy)
        self.assertIn('mode: "canonical"', self.policy)
        self.assertIn('expected_policy(self.is_ready())', self.policy)
        self.assertIn('standalone_cannot_reuse_a_previously_installed_canonical_policy', self.policy)

    def test_schema_rebuild_prepares_before_returning_to_runtime(self):
        provisioner = (ROOT / 'backend/examples/browser_fixture_provisioner.rs').read_text()
        rebuild = provisioner.split('async fn catalogue_rebuild', 1)[1].split('async fn recipient_setup', 1)[0]
        ordered = ('.migrate(weebdex_platform::domains::MIGRATIONS)', 'protocol.verify_schema().await?',
                   'protocol.install_visibility_function().await?', 'protocol.verify_visibility_function().await?')
        self.assertEqual(sorted(rebuild.index(x) for x in ordered), [rebuild.index(x) for x in ordered])
        fixture = (ROOT / 'tests/runtime/browser_fixture.py').read_text()
        start = fixture.split('    def start(self):', 1)[1].split('    @staticmethod', 1)[0]
        self.assertLess(start.index("('original-schema-and-authorities',self.protocol_setup)"),
                        start.index("('platform-migrations',self.migrate)"))
        self.assertIn("[str(self.args.binary),'migrate']", fixture)

    def test_runtime_role_guidance_excludes_history_and_policy_writes(self):
        docs = (ROOT / 'docs/production-readiness.md').read_text()
        for needed in ('SELECT only on both migration histories', 'No schema CREATE',
                       'membership/SET ROLE path', 'not an automatically applied',
                       'A restored checkpoint is verified as-is', 'two-role PostgreSQL'):
            self.assertIn(needed, docs)


if __name__ == '__main__':
    unittest.main()
