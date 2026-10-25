from pathlib import Path
import json,subprocess,hashlib,re
W=Path.cwd();base='b4feb0b02350ac5332a59a47a308540fe9c4cdf7';head='b72c91d08bb105ed8f9be74002579b1fc9fa9764';D=Path(__file__).resolve().parent
def git(*a):return subprocess.check_output(['git',*a])
def blob(c,p):return git('show',f'{c}:{p}')
def sha(b):return hashlib.sha256(b).hexdigest()
originals=['c056047d95f5d5b91c10db155cff1636e6dcd28b','e47b33aeb371dc08f4486ca86217e52cde7e1227','13ff71d093c14d5d9a0a8f437f8a681f0237978b','c40e813a5b2e7342e1a6ac9a6eb6a32457f812c3','ad6c76bf0b39cd96b432e69eb79f41280f0d48eb','93ff312704d2f750cbd166e9c20984f6cf7bf04d','31396b44e0abbb4d9fc3689545a5cd2425b8879b','db5964cffda71e89a95395b1a03346b02dced04e','0cf2b60bb08c00bb3090bbbc519c57466b74cf74','df4adab27bfdc6fbc5e327d5d1dd33b7186a34da','ed4935dceaa8140825bed1c5c3a6a9a32cf0a796','0e978d36c57d935f830650248a442fa20a923387','ecd842cc53ed2b42a17f0fd279a8ba328d139d44','729ce95873a1d5594f5629a908f9c247fc4727b6','b76308ad1e42650c2acbb31ba443445e0004e328','b22de61e695a427fcbceac1a0ccb2741f728bd58','1f5059d48c7745c77ae2982833f0c21617a845af']
local=git('rev-list','--reverse',f'{base}..{head}').decode().splitlines();mapping=[];by_subject={git('show','-s','--format=%s',c).decode().strip():c for c in originals}
for c in local:
 subject=git('show','-s','--format=%s',c).decode().strip();mapping.append({'applied':c,'original':by_subject.get(subject),'subject':subject,'compositionOnly':subject not in by_subject})
assert len(mapping)==19 and len([m for m in mapping if m['original']])==17
assert sorted(m['original'] for m in mapping if m['original'])==sorted(originals)
paths=git('ls-tree','-r','--name-only',base).decode().splitlines();changed=[];unchanged=[]
for p in paths:
 actual=(W/p).read_bytes();before=blob(base,p)
 (unchanged if before==actual else changed).append(p)
allowed=set()
for c in originals:allowed.update(git('diff-tree','--no-commit-id','--name-only','-r',c).decode().splitlines())
allowed.add('weebdex-platform/scripts/reconstruct-public-api-anchors.mjs')
assert set(changed)<=allowed,sorted(set(changed)-allowed)
critical=[p for p in paths if p.startswith('weebdex-platform/backend/migrations/') or any(k in p for k in ['/public-api/cover-client.ts','/public-api/PublicCoverEditor.svelte','/public-api/PublicTitleEditor.svelte','/public-api/PublicTitleDisposition.svelte','/public_api/cover_writes.rs','/public_api/manga_disposition.rs','/public-api/library-catalogue.ts','/local-library/','/public-api/settings-','/public-api/AccountPreferenceSave.svelte','/public-api/list-follow-','/public-api/public-lists.ts','/public-api/AccountFollow.svelte','/public-api/account-follow-'])]
assert all(p in unchanged for p in critical),[p for p in critical if p not in unchanged]
A='weebdex-platform/platform-integrations/public-api/anchors.json';old=json.loads(blob(base,A));now=json.loads((W/A).read_bytes());assert len(old)==60 and len(now)==64
for i,p in enumerate(old):
 expected=dict(p)
 if expected.get('approved_public_availability_exception')=='public_group_claim_requires_command_adapter':expected['approved_public_availability_exception']='public_group_claim_requires_exact_review_adapter'
 assert expected==now[i],i
P='weebdex-platform/platform-integrations/public-api/source-hashes.json';op=json.loads(blob(base,P));np=json.loads((W/P).read_bytes());assert len(op)==40 and len(np)==42 and all(np[p]==v for p,v in op.items())
# Source-level preservation of every prior test title. Existing matching assertions
# that deliberately gained signed expectedAccountId are identified by the diff.
T='weebdex-platform/tests/ci/test_public_title_commands.mjs';titles=lambda b:set(re.findall(rb"test\('([^']+)'",b));assert titles(blob(base,T))<=titles((W/T).read_bytes())
F='weebdex-platform/test-evidence/frontend/integration-source-manifest.json';assert F in unchanged
check=subprocess.run(['git','diff','--check',base,'--',':!weebdex-platform/test-evidence/public-api/account-intent-audit-20261017/uncompiled-baseline-probe.patch'],capture_output=True,text=True);assert check.returncode==0,check.stdout+check.stderr
allcheck=subprocess.run(['git','diff','--check',base],capture_output=True,text=True)
assert allcheck.returncode==2 and 'uncompiled-baseline-probe.patch:6: trailing whitespace.' in allcheck.stdout
result={'schema':'guard-editorial-source-composition-v1','base':base,'sourceHead':head,'sourceOnly':True,'appliedMap':mapping,'baseTrackedFiles':len(paths),'unchangedBaseFiles':len(unchanged),'changedBaseFiles':changed,'removedBaseFiles':[],'criticalByteIdenticalFiles':{p:sha((W/p).read_bytes()) for p in critical},'allExistingAccountLibraryMigrationsAndCoverDispositionControlsPreserved':True,'canonicalInputPins':{'before':len(op),'after':len(np),'allExistingExact':True},'anchors':{'before':len(old),'after':len(now),'retainedBaseRecipes':60,'deliberateLabelOnlyChanges':[24,25],'newEditorialRecipes':4},'titleTestNamesPreserved':len(titles(blob(base,T))),'sourceDiffCheck':'passed excluding exact historical baseline-probe.patch bytes','carriedHistoricalPatchWhitespace':{'path':'weebdex-platform/test-evidence/public-api/account-intent-audit-20261017/uncompiled-baseline-probe.patch','line':6,'sha256':sha((W/'weebdex-platform/test-evidence/public-api/account-intent-audit-20261017/uncompiled-baseline-probe.patch').read_bytes()),'reason':'Immutable pre-fix diagnostic patch retained verbatim; its context blank line has one space.'},'newCombinedQualification':{'strict':False,'rust':False,'http':False,'productionBuild':False,'browser':False},'generatorParity':json.loads((D/'final-generator-dry.json').read_text()),'historicalManifestScope':json.loads((W/'weebdex-platform/test-evidence/public-api/editorial-tools/historical-preparation-scope.json').read_text()),'generatorFirstAttemptFailure':{'observed':True,'rawLogRetained':False,'detail':'Retaining existing composed User.svelte recipe with raw-source-only uniqueness check failed. Production stage-aware validation now checks actual preceding native/reader/library stages and preceding public patches; exact recipes unchanged.'}}
(D/'composition-map-and-preservation.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'head':head,'baseTrackedFiles':len(paths),'unchanged':len(unchanged),'changed':len(changed),'criticalByteIdentical':len(critical),'appliedOriginals':17,'compositionOnlyCommits':2,'anchors':len(now),'pins':len(np),'sourceDiffCheck':'passed with one documented historical evidence whitespace exclusion'}))
