import json,pathlib,sys,unittest
out=pathlib.Path(__file__).resolve().parent;rows=json.loads((out/'selected-files.json').read_text())['modules'];suite=unittest.TestSuite();counts={}
for row in rows:
 name=pathlib.Path(row['path']).stem;group=unittest.defaultTestLoader.loadTestsFromName(name);assert group.countTestCases()==row['expectedUnittestMethods'],name;counts[name]=group.countTestCases();suite.addTests(group)
result=unittest.TextTestRunner(verbosity=2).run(suite);report={'files':counts,'testsRun':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'status':'passed'if result.wasSuccessful()else'failed'};(out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(not result.wasSuccessful())
