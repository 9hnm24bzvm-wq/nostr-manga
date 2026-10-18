// Reuse an existing TypeScript installation; no dependency installation/copy.
const ts=require(process.env.WEEBDEX_TYPESCRIPT_ROOT);
const options={noEmit:true,strict:true,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,skipLibCheck:true};
const program=ts.createProgram(['weebdex-platform/platform-integrations/public-api/catalogue-edges.ts'],options);
const diagnostics=ts.getPreEmitDiagnostics(program);
console.log(ts.formatDiagnosticsWithColorAndContext(diagnostics,{getCurrentDirectory:()=>process.cwd(),getCanonicalFileName:file=>file,getNewLine:()=> '\n'}));
console.log('diagnostics',diagnostics.length);process.exitCode=diagnostics.length?1:0;
