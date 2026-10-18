"""Bounded finite-only diagnostics; no browser, service or database."""
import pathlib,subprocess,sys,unittest
root=pathlib.Path(__file__).resolve().parents[4]
patterns=['test_runner_diagnostics.py','test_hosted_browser.py','test_public_upload_hosted_profile.py','test_native_feed_diagnostics.py','test_journey_diagnostics.py']
suite=unittest.TestSuite()
for pattern in patterns: suite.addTests(unittest.defaultTestLoader.discover(str(root/'weebdex-platform/tests/ci'),pattern=pattern))
r=unittest.TextTestRunner(verbosity=2).run(suite)
if not r.wasSuccessful():sys.exit(1)
for command in [['node','weebdex-platform/tests/ci/test_runner_lifecycle.mjs'],['node','--test','--test-name-pattern=current actor-follow runtime|supplemental source plan','weebdex-platform/tests/ci/test_browser_actor_follow_journeys.mjs']]:
 result=subprocess.run(command,cwd=root)
 if result.returncode:sys.exit(result.returncode)
