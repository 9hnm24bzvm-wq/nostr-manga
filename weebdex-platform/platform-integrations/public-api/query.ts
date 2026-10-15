// RECONSTRUCTED from retained contract/tests, not recovered original query source.
// Every unsupported control fails; no caller filter is silently dropped.
import { publicUuid,PublicApiContractError } from './validation';
export type QueryFamily='manga'|'chapter'|'aggregate'|'cover'|'author'|'group'|'tag';
const fail=(message:string):never=>{throw new PublicApiContractError(message,422);};
const ratings=['safe','suggestive','erotica','pornographic'],statuses=['ongoing','completed','hiatus','cancelled'],demographics=['shounen','shoujo','josei','seinen','none'];
const idArrays=new Set(['ids[]','authors[]','artists[]','includedTags[]','excludedTags[]','groups[]','excludedGroups[]','uploader[]','excludedUploaders[]','manga[]']);
const scalarKeys=new Set(['title','name','limit','offset','year','hasAvailableChapters','includedTagsMode','excludedTagsMode','authorOrArtist','inactive','focusedLanguage','translatedLanguage','includeFuturePublishAt','includeEmptyPages','includeFutureUpdates','includeUnavailable','createdAtSince','updatedAtSince','publishAtSince','volume','chapter']);
const arrays=new Set([...idArrays,'originalLanguage[]','excludedOriginalLanguage[]','availableTranslatedLanguage[]','translatedLanguage[]','excludedTranslatedLanguage[]','contentRating[]','status[]','publicationDemographic[]','includes[]']);
const aliases:Record<string,string>={id:'ids[]',author:'authors[]',artist:'artists[]',tag:'includedTags[]',tagx:'excludedTags[]',group:'groups[]',groupx:'excludedGroups[]',uploader:'uploader[]',uploaderx:'excludedUploaders[]',manga:'manga[]',lang:'originalLanguage[]',langx:'excludedOriginalLanguage[]',tlang:'translatedLanguage[]',contentRating:'contentRating[]',status:'status[]',demographic:'publicationDemographic[]',hasChapters:'hasAvailableChapters'};
const sortMap:Record<string,string>={title:'title',createdAt:'createdAt',updatedAt:'updatedAt',lastUploadedChapterAt:'latestUploadedChapter',latestUploadedChapter:'latestUploadedChapter',chapter:'chapter',volume:'volume',publishAt:'publishAt',readableAt:'readableAt',name:'name',relevance:'relevance',followedCount:'followedCount',rating:'rating',year:'year'};
export function publicQuery(family:QueryFamily,input:URLSearchParams,includes:readonly string[]=[]):URLSearchParams {
 const out=new URLSearchParams(),seen=new Set<string>();let page:string|undefined,sort:string|undefined,direction:string|undefined,yearFrom:string|undefined,yearTo:string|undefined;
 for(const[original,value]of input){let key=original.endsWith('[]')?original.slice(0,-2):original;if(key==='tlangx')fail('Translated-language exclusion is unavailable; the filter was not dropped.');if(['page','sort','order','yearFrom','yearTo','tmod','txmod'].includes(key)){if(seen.has(key))fail('Duplicate search option '+key);seen.add(key);if(key==='page')page=value;else if(key==='sort')sort=value;else if(key==='order')direction=value;else if(key==='yearFrom')yearFrom=value;else if(key==='yearTo')yearTo=value;else{const mode=key==='tmod'?(value==='0'?'AND':value==='1'?'OR':value):(value==='0'?'OR':value==='1'?'AND':value);if(!['AND','OR'].includes(mode))fail('Unsupported tag mode');out.set(key==='tmod'?'includedTagsMode':'excludedTagsMode',mode);}continue;}
  key=aliases[key]??(arrays.has(original)?original:key);if(family==='manga'&&key==='translatedLanguage[]')key='availableTranslatedLanguage[]';
  if(/^order\[[A-Za-z]+\]$/.test(key)){const field=key.slice(6,-1);if(!Object.values(sortMap).includes(field)||!['asc','desc'].includes(value)||seen.has(key))fail('Unsupported or duplicate order');seen.add(key);out.append(key,value);continue;}
  if(!arrays.has(key)&&!scalarKeys.has(key))fail('Search option unavailable: '+original+'. The filter was not dropped.');if(!arrays.has(key)){if(seen.has(key))fail('Duplicate search option '+key);seen.add(key);}
  if(idArrays.has(key)||key==='authorOrArtist')publicUuid(value);
  if(key==='contentRating[]'&&!ratings.includes(value))fail('Invalid content rating');if(key==='status[]'&&!statuses.includes(value))fail('Invalid manga status');if(key==='publicationDemographic[]'&&!demographics.includes(value))fail('Invalid demographic');
  if(['limit','offset','year'].includes(key)){if(!/^\d+$/.test(value)||!Number.isSafeInteger(Number(value))||Number(value)<(key==='limit'?1:0))fail('Invalid '+key);}
  if(key==='hasAvailableChapters'||key==='inactive'){if(!['true','false'].includes(value))fail('Invalid boolean '+key);}
  if(key==='includedTagsMode'||key==='excludedTagsMode'){if(!['AND','OR'].includes(value))fail('Invalid tag mode');}
  if(!value)fail('Empty search option '+key);out.append(key,value);
 }
 if(yearFrom!==undefined||yearTo!==undefined){if(yearFrom===undefined||yearTo===undefined||yearFrom!==yearTo||out.has('year'))fail('Year ranges are unavailable; the filter was not dropped.');if(!/^\d{4}$/.test(yearFrom))fail('Invalid year');out.set('year',yearFrom);}
 if(page!==undefined){if(!/^\d+$/.test(page)||Number(page)<1||out.has('offset'))fail('Invalid or ambiguous page/offset');const offset=(Number(page)-1)*Number(out.get('limit')??100);if(!Number.isSafeInteger(offset))fail('Page offset exceeds budget');out.set('offset',String(offset));}
 if(sort!==undefined){const field=sortMap[sort];if(!field)fail('Unsupported sort: '+sort);if(out.has('order['+field+']'))fail('Duplicate sort field');if(direction!==undefined&&!['asc','desc'].includes(direction))fail('Invalid order direction');out.append('order['+field+']',direction??'desc');}else if(direction!==undefined){if(!['asc','desc'].includes(direction))fail('Invalid order direction');if(family!=='chapter'&&family!=='aggregate')fail('Order requires a known sort');out.append('order[chapter]',direction);}
 for(const include of includes)if(!out.getAll('includes[]').includes(include))out.append('includes[]',include);
 if(family==='tag'&&[...out.keys()].some(key=>key!=='includes[]'))fail('Tag search controls have no verified public contract');return out;
}
