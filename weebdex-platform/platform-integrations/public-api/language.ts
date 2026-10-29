// Public wire syntax is not the native Locale registry. Read labels stay exact.
import { PublicApiContractError } from './validation';
export const publicWriteLanguagePattern = /^[a-z]{2}(-[a-z]{2})?$/;
export const publicSearchLanguagePattern = /^[a-zA-Z\-]{2,5}$/;
export function publicWriteLanguage(value: unknown): string {
 if (typeof value !== 'string' || publicWriteLanguagePattern.exec(value)?.[0] !== value) throw new PublicApiContractError('Use a lowercase two-letter public language code, optionally followed by a two-letter region (for example ta or pt-br).', 422);
 return value;
}
export function publicSearchLanguage(value: string): boolean { return publicSearchLanguagePattern.exec(value)?.[0] === value; }
// Complete editor state is never itself a patch. Unchanged imported labels,
// including native-only codes, must not become new public writes.
export function chapterMetadataPatch(current: {version:number; title?:string|null; volume?:string|null; chapter?:string|null; language:string; relationships:{groups?:{id:string}[]}}, fields: {title:string;volume:string;chapter:string;language:string;groups:string[]}): Record<string,unknown> {
 const out:Record<string,unknown>={version:current.version};
 for(const key of ['title','volume','chapter','language'] as const) if(fields[key] !== (current[key]??'')) out[key]=key==='language'?publicWriteLanguage(fields[key]):fields[key];
 if(JSON.stringify(fields.groups)!==JSON.stringify((current.relationships.groups??[]).map(group=>group.id)))out.groups=[...fields.groups];
 return out;
}

// Canonical choices for new writes, while imported values remain untouched.
export function canonicalLanguageOptions(registry:Record<string,string>):[string,string][] {
 return Object.entries({...registry,ta:'Tamil'}).filter(([code])=>publicWriteLanguagePattern.test(code)).sort((a,b)=>a[1].localeCompare(b[1]));
}
export function languageLabel(value:string,registry:Record<string,string>):string { return value==='tam'?'Tamil (previously saved code: tam)':value==='ta'?'Tamil':registry[value]??value; }
export const legacyLanguageReplacement=(value:string):string|undefined=>value==='tam'?'ta':undefined;
