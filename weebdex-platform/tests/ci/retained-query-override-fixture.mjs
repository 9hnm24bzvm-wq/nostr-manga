// Execute the actual retained layout prototype hook, not a reimplementation.
import fs from 'node:fs';import vm from 'node:vm';import{stripTypeScriptTypes}from'node:module';
export function installRetainedQueryOverride(){
 const original=URLSearchParams.prototype.toString,source=fs.readFileSync(new URL('../../frontend/src/routes/+layout.ts',import.meta.url),'utf8');
 const start=source.indexOf('const _toString = URLSearchParams.prototype.toString;'),end=source.indexOf('Number.prototype.short =',start);
 if(start<0||end<=start)throw Error('Retained query hook changed');
 vm.runInNewContext(stripTypeScriptTypes(source.slice(start,end),{mode:'transform'}),{URLSearchParams});
 if(new URLSearchParams({probe:'x'}).toString()!=='?probe=x')throw Error('Retained query hook did not execute');
 return()=>{URLSearchParams.prototype.toString=original;};
}
export function assertCanonicalQuery(assert,input,base='https://catalog.example'){
 const value=new URL(input,base);assert.ok(!value.search.startsWith('??'),'double-prefixed query: '+value.href);
 for(const name of value.searchParams.keys())assert.ok(!name.startsWith('?'),'unknown prefixed query key: '+name);
 return value;
}
