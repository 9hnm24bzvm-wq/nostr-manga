"""Only affected finite diagnostics and closed hosted-profile contracts."""
import pathlib,sys,unittest
root=pathlib.Path(__file__).resolve().parents[3]
patterns=['test_native_feed_diagnostics.py','test_public_upload_hosted_profile.py','test_runner_diagnostics.py','test_journey_diagnostics.py']
suite=unittest.TestSuite()
for pattern in patterns:
 suite.addTests(unittest.defaultTestLoader.discover(str(root/'weebdex-platform/tests/ci'),pattern=pattern))
result=unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(0 if result.wasSuccessful() else 1)
