// RECONSTRUCTED 2026-10-15. Retest before release; product activation stays off.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const sourceHashes=JSON.parse(fs.readFileSync(new URL('./source-hashes.json',import.meta.url),'utf8'));
const patches=JSON.parse(fs.readFileSync(new URL('./anchors.json',import.meta.url),'utf8'));
const hash=value=>createHash('sha256').update(value).digest('hex');
const apiFamilies=['Manga','Chapter','Cover','Author','Group','Tag','Statistics','Analytics'];
// Explicit finite coverage. Other catalogue methods are blocked when enabled.
const readMethods={
 Manga:{Search:['manga.search','MangaFeedResponse','query: searchParams'],Get:['manga.get','Manga','id'],Random:['manga.random','Manga','query: searchParams'],Trending:['manga.trending','MangaFeedResponse','query: searchParams'],Relations:['manga.relations','Relation[]','id'],Recommendations:['manga.recommendations','MangaFeedResponse','id']},
 Chapter:{Search:['chapter.search','MappedChapterFeedResponse','query: searchParams'],LatestUpdates:['chapter.latestUpdates','ChapterFeedResponse','query: searchParams'],LatestFeed:['chapter.latestFeed','MappedChapterFeedResponse','query: searchParams'],Trending:['chapter.trending','MappedChapterFeedResponse','query: searchParams'],Get:['chapter.get','Chapter','id'],List:['chapter.list','ChapterFeedResponse','id: mangaId, query: searchParams'],Aggregate:['chapter.aggregate','ChapterAggregateResponse','id: mangaId, query: searchParams']},
 Cover:{List:['cover.list','Cover[]','id: mangaId'],Get:['cover.get','Cover','id']},Author:{Search:['author.search','TagFeedResponse','query: searchParams'],Get:['author.get','Author','id']},Group:{Search:['group.search','ScanlationGroupFeedResponse','query: searchParams'],Get:['group.get','ScanlationGroup','id']},Tag:{Search:['tag.search','TagFeedResponse','query: searchParams']},Statistics:{Manga:['statistics.manga','MangaStats','id'],Chapter:['statistics.chapter','ChapterStats','id'],Group:['statistics.group','ScanlationGroupStats','id']},Analytics:{Manga:['analytics.manga','unknown','id'],Chapter:['analytics.chapter','unknown','id']}
};
export function readPublicApiGate(value=process.env.PUBLIC_MANGADEX_CATALOGUE){if(value===undefined||value==='0')return false;if(value==='1')return true;throw Error('PUBLIC_MANGADEX_CATALOGUE must be 0 or 1');}
export const publicApiOverlayManifest=Object.entries(sourceHashes).map(([relative,sha256])=>({path:relative,sha256,public_api_base:'/api/v1',activation:'explicit staged gate; default off',reconstructed_unverified:true}));
export const publicApiApprovedMediaExceptions=patches.filter(entry=>entry.approved_public_media_exception);
export const publicApiApprovedAvailabilityExceptions=patches.filter(entry=>entry.approved_public_availability_exception);
export function assertPublicApiExceptionScope(){const media={'src/routes/(app)/popular/MostReadChapter.svelte':1,'src/routes/(app)/title/Manga.svelte':1,'src/routes/(app)/title/MangaCoversSection.svelte':1};if(publicApiApprovedMediaExceptions.length!==3||publicApiApprovedMediaExceptions.some(entry=>entry.count!==1||entry.approved_public_media_exception!=='uuid_cover_url_expression'||!media[entry.path])||Object.keys(media).some(relative=>publicApiApprovedMediaExceptions.filter(entry=>entry.path===relative).length!==1))throw Error('Public media exception scope changed');const availability={'src/routes/(app)/title/Manga.svelte':'missing_native_comments_statistics','src/lib/components/ChapterListEntry.svelte':'actual_native_forum_context_only','src/lib/components/ChapterFeedChapterEntry.svelte':'actual_native_forum_context_only','src/lib/components/reader/reader.svelte':'missing_native_forum_context'};if(publicApiApprovedAvailabilityExceptions.length!==4||publicApiApprovedAvailabilityExceptions.some(entry=>entry.count!==1||availability[entry.path]!==entry.approved_public_availability_exception||Boolean(entry.composed_before)!==entry.path.endsWith('/reader.svelte'))||Object.keys(availability).some(relative=>publicApiApprovedAvailabilityExceptions.filter(entry=>entry.path===relative).length!==1))throw Error('Public availability exception scope changed');}
function replaceExact(code,entry){if(code.split(entry.before).length-1!==entry.count)throw Error(`Public API overlay anchor changed: ${entry.path}`);return code.split(entry.before).join(entry.after);}
export function createPublicApiOverlays(frontend,{enabled=readPublicApiGate()}={}){
 if(typeof enabled!=='boolean')throw Error('Public API overlay enabled must be boolean');const root=path.resolve(frontend);assertPublicApiExceptionScope();
 return {name:'retained-public-uuid-catalogue-staging',enforce:'pre',transform:{order:'pre',handler(input,id){
  // Svelte virtual CSS must never be treated as original source.
  if(!enabled||id.includes('?'))return null;const relative=path.relative(root,id).split(path.sep).join('/');if(!Object.hasOwn(sourceHashes,relative))return null;
  if(hash(fs.readFileSync(path.join(root,relative),'utf8'))!==sourceHashes[relative])throw Error(`Public API attachment source hash changed: ${relative}`);
  let code=input;const family=apiFamilies.find(name=>relative===`src/lib/API/${name}.ts`);
  if(family){const methods=[...code.matchAll(/export const (\w+) = ([^\r\n]*?) => /g)];if(methods.length!==(code.match(/export const /g)??[]).length)throw Error(`Unreviewed API method signature: ${relative}`);for(const match of methods.reverse()){const name=match[1];if(family==='Statistics'&&['User','Thread'].includes(name))continue;const read=readMethods[family]?.[name],hasSignal=/\bsignal\??\s*:/.test(match[2]),statement=read?`return publicCatalogueRead<${read[1]}>(${JSON.stringify(read[0])}, { ${read[2]}${hasSignal?', signal':''} });`:`return blockNativeCatalogueWrite(${JSON.stringify(`${family}.${name}`)});`;const position=match.index+match[0].length;if(code[position]==='{')code=code.slice(0,position+1)+'\n\tif (Boolean(true)) '+statement+code.slice(position+1);else{const end=code.indexOf(';',position);if(end<0)throw Error(`Unterminated API expression: ${relative}.${name}`);code=code.slice(0,position)+`{ if (Boolean(true)) ${statement} return `+code.slice(position,end+1)+' }'+code.slice(end+1);}}
   code='import { publicCatalogueRead, blockNativeCatalogueWrite } from "$publicApi/site-client";\n'+code;
  }
  const relevant=patches.filter(entry=>entry.path===relative);for(const entry of relevant)code=replaceExact(code,entry);
  const helpers=[...new Set(relevant.map(entry=>entry.helper).filter(Boolean))];for(const helper of helpers){const names=helper==='media'?'retainedCoverUrl, retainedPageUrl':'retainedMangaIsEmpty';const script=/<script\b[^>]*>/;if(!script.test(code))throw Error(`Missing original script for public helper: ${relative}`);code=code.replace(script,opening=>`${opening}\n\timport { ${names} } from "$publicApi/${helper}";`);}
  return code===input?null:{code,map:null};
 }}};
}
