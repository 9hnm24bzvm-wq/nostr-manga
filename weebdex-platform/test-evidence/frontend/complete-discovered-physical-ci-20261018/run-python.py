import json,pathlib,sys,unittest
out=pathlib.Path(__file__).resolve().parent
files=json.loads((out/'finite-python-files.json').read_text());suite=unittest.TestSuite();counts={}
for file in files:
 path=pathlib.Path(file);group=unittest.defaultTestLoader.discover(str(path.parent),pattern=path.name);counts[file]=group.countTestCases();suite.addTests(group)
result=unittest.TextTestRunner(verbosity=2).run(suite)
report={'files':counts,'testsRun':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'status':'passed' if result.wasSuccessful() else 'failed'}
(out/'python-results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(not result.wasSuccessful())
