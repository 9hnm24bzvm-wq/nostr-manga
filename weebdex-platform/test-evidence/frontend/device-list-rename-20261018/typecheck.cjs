const ts=require(process.env.WEEBDEX_TYPESCRIPT_ROOT),noble=process.env.WEEBDEX_MIGRATION_TEST_NOBLE;
const options={noEmit:true,strict:true,allowJs:true,checkJs:true,skipLibCheck:true,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,paths:{'@noble/hashes':[noble],'@noble/hashes/*':[noble+'/*']}};
const program=ts.createProgram(['weebdex-platform/platform-integrations/local-library/device-list-name.ts'],options),diagnostics=ts.getPreEmitDiagnostics(program);
console.log(ts.formatDiagnosticsWithColorAndContext(diagnostics,{getCurrentDirectory:()=>process.cwd(),getCanonicalFileName:file=>file,getNewLine:()=> '\n'}));console.log('diagnostics',diagnostics.length);process.exitCode=diagnostics.length?1:0;
