import { publicSearchLanguage, publicWriteLanguage } from './language';
// Operation-specific translation of retained view controls to the pinned 5.13.1
// wire shapes. Unsupported controls fail before transport; none are dropped.
import {publicUuid,PublicApiContractError} from './validation';
export type QueryFamily='manga'|'random'|'chapter'|'feed'|'chapterView'|'aggregate'|'cover'|'author'|'group'|'tag';
const mangaScalar=['title','authorOrArtist','year','includedTagsMode','excludedTagsMode','createdAtSince','updatedAtSince','hasAvailableChapters','hasUnavailableChapters','group'];
const mangaArrays=['authors[]','artists[]','includedTags[]','excludedTags[]','status[]','originalLanguage[]','excludedOriginalLanguage[]','availableTranslatedLanguage[]','publicationDemographic[]','ids[]','contentRating[]'];
const chapterScalar=['title','manga','includeFutureUpdates','includeEmptyPages','includeFuturePublishAt','includeExternalUrl','externalUrl','excludeExternalUrl','includeUnavailable','createdAtSince','updatedAtSince','publishAtSince'];
const chapterArrays=['ids[]','groups[]','uploader','includes','volume[]','chapter','translatedLanguage[]','originalLanguage[]','excludedOriginalLanguage[]','contentRating[]','excludedGroups[]','excludedUploaders[]'];
const feedScalar=chapterScalar.filter(key=>!['title','manga'].includes(key));
const feedArrays=['translatedLanguage[]','originalLanguage[]','excludedOriginalLanguage[]','contentRating[]','excludedGroups[]','excludedUploaders[]','includes[]'];
export const chapterParentControls=new Set(['authorOrArtist','authors[]','artists[]','includedTags[]','excludedTags[]','includedTagsMode','excludedTagsMode','publicationDemographic[]','status[]','year','hasAvailableChapters','hasUnavailableChapters','availableTranslatedLanguage[]']);
const schema:Record<QueryFamily,{scalar:readonly string[];arrays:readonly string[];order:readonly string[];includes:readonly string[]}>={
 manga:{scalar:mangaScalar,arrays:[...mangaArrays,'includes[]'],order:['title','year','createdAt','updatedAt','latestUploadedChapter','followedCount','relevance','rating'],includes:['manga','cover_art','author','artist']},
 random:{scalar:['includedTagsMode','excludedTagsMode'],arrays:['contentRating[]','includedTags[]','excludedTags[]','includes[]'],order:[],includes:['manga','cover_art','author','artist']},
 chapter:{scalar:chapterScalar,arrays:chapterArrays,order:['createdAt','updatedAt','publishAt','readableAt','volume','chapter'],includes:['manga','scanlation_group','user']},
 feed:{scalar:feedScalar,arrays:feedArrays,order:['createdAt','updatedAt','publishAt','readableAt','volume','chapter'],includes:['manga','scanlation_group','user']},
 // An internal composition input, never sent directly to an endpoint.
 chapterView:{scalar:[...chapterScalar,...mangaScalar.filter(key=>key!=='group')],arrays:[...chapterArrays,...mangaArrays],order:['createdAt','updatedAt','publishAt','readableAt','volume','chapter'],includes:['manga','scanlation_group','user']},
 aggregate:{scalar:['includeUnavailable'],arrays:['translatedLanguage[]','groups[]'],order:[],includes:[]},
 cover:{scalar:[],arrays:['ids[]','manga[]','uploaders[]','locales[]','includes[]'],order:['createdAt','updatedAt','volume'],includes:['manga','user']},
 author:{scalar:['name'],arrays:['ids[]','includes[]'],order:['name'],includes:['manga']},
 group:{scalar:['name','focusedLanguage'],arrays:['ids[]','includes[]'],order:['name','createdAt','updatedAt','followedCount','relevance'],includes:['leader','member']},
 tag:{scalar:[],arrays:[],order:[],includes:[]}
};
const idKeys=new Set(['ids[]','authors[]','artists[]','includedTags[]','excludedTags[]','groups[]','excludedGroups[]','uploader','uploaders[]','excludedUploaders[]','manga[]','manga','group','authorOrArtist']);
const aliases:Record<string,string>={availableTranslatedLang:'availableTranslatedLanguage[]',artists:'artists[]',id:'ids[]',author:'authors[]',artist:'artists[]',tag:'includedTags[]',tagx:'excludedTags[]',groupx:'excludedGroups[]',uploaderx:'excludedUploaders[]',lang:'originalLanguage[]',langx:'excludedOriginalLanguage[]',tlang:'translatedLanguage[]',contentRating:'contentRating[]',status:'status[]',demographic:'publicationDemographic[]',hasChapters:'hasAvailableChapters',chapter:'chapter',volume:'volume[]'};
const sortMap:Record<string,string>={lastUploadedChapterAt:'latestUploadedChapter',follows:'followedCount'};
function fail(message:string):never{throw new PublicApiContractError(message,422);}
export function publicQuery(family:QueryFamily,input:URLSearchParams,includes:readonly string[]=[]):URLSearchParams{
 const definition=schema[family],out=new URLSearchParams(),seen=new Set<string>(),counts=new Map<string,number>(),collection=!['random','aggregate','tag'].includes(family);
 let page:string|undefined,sort:string|undefined,direction:string|undefined,yearFrom:string|undefined,yearTo:string|undefined;
 function append(key:string,value:string){
  const ordered=/^order\[([A-Za-z]+)\]$/.exec(key);
  if(ordered){if(!definition.order.includes(ordered[1])||!['asc','desc'].includes(value)||seen.has(key))fail('Unsupported or duplicate order: '+key);seen.add(key);out.append(key,value);return;}
  const array=definition.arrays.includes(key);
  if(!array&&!definition.scalar.includes(key)&&!(collection&&['limit','offset'].includes(key)))fail('Search option unavailable for '+family+': '+key+'. The filter was not dropped.');
  if(!array){if(seen.has(key))fail('Duplicate search option '+key);seen.add(key);}else{const count=(counts.get(key)??0)+1;if(count>100)fail('Search array exceeds 100 values');counts.set(key,count);}
  if(idKeys.has(key))value=publicUuid(value);
  if(['originalLanguage[]','excludedOriginalLanguage[]','availableTranslatedLanguage[]','translatedLanguage[]'].includes(key)&&!publicSearchLanguage(value))fail('Invalid language search filter');
  if(key==='locales[]')publicWriteLanguage(value);
  // focusedLanguage is a bounded opaque group filter, not a write-language code.
  if(key==='focusedLanguage'&&new TextEncoder().encode(value).byteLength>4096)fail('Group language filter exceeds its text budget');
  if(key==='contentRating[]'&&!['safe','suggestive','erotica','pornographic'].includes(value))fail('Invalid content rating');
  if(key==='status[]'&&!['ongoing','completed','hiatus','cancelled'].includes(value))fail('Invalid manga status');
  if(key==='publicationDemographic[]'&&!['shounen','shoujo','josei','seinen','none'].includes(value))fail('Invalid demographic');
  if(key==='limit'||key==='offset'){const maximum=key==='offset'?10000:family==='feed'?500:100,minimum=key==='limit'&&family==='feed'?1:0;if(!/^\d+$/.test(value)||!Number.isSafeInteger(Number(value))||Number(value)<minimum||Number(value)>maximum)fail('Invalid '+key);}
  if(key==='year'&&value!=='none'&&(!/^\d+$/.test(value)||!Number.isSafeInteger(Number(value))))fail('Invalid year');
  if(key==='hasAvailableChapters'||key==='hasUnavailableChapters'){if(['0','1'].includes(value))value=value==='1'?'true':'false';if(!['true','false'].includes(value))fail('Invalid boolean '+key);}
  if(key.startsWith('include')&&['includeFutureUpdates','includeEmptyPages','includeFuturePublishAt','includeExternalUrl','includeUnavailable'].includes(key)){if(['true','false'].includes(value))value=value==='true'?'1':'0';if(!['0','1'].includes(value))fail('Invalid availability flag '+key);}
  if(['includedTagsMode','excludedTagsMode'].includes(key)&&!['AND','OR'].includes(value))fail('Invalid tag mode');
  if((key==='includes'||key==='includes[]')&&!definition.includes.includes(value))fail('Unsupported relationship include');
  if(!value)fail('Empty search option '+key);out.append(key,value);
 }
 for(const[original,value]of input){const bare=original.endsWith('[]')?original.slice(0,-2):original;
  if(bare==='tlangx')fail('Translated-language exclusion has no verified public contract. The filter was not dropped.');
  if(['page','sort','order','yearFrom','yearTo','tmod','txmod'].includes(bare)){if(seen.has(bare))fail('Duplicate search option '+bare);seen.add(bare);if(bare==='page')page=value;else if(bare==='sort')sort=value;else if(bare==='order')direction=value;else if(bare==='yearFrom')yearFrom=value;else if(bare==='yearTo')yearTo=value;else append(bare==='tmod'?'includedTagsMode':'excludedTagsMode',value==='0'?(bare==='tmod'?'AND':'OR'):value==='1'?(bare==='tmod'?'OR':'AND'):value);continue;}
  let key=aliases[bare]??(definition.arrays.includes(original)?original:bare);
  if(bare==='group')key=family==='manga'?'group':'groups[]';
  if(bare==='manga')key=family==='cover'?'manga[]':'manga';
  if(bare==='uploader'||bare==='uploaders')key=family==='cover'?'uploaders[]':'uploader';
  if(bare==='locales')key='locales[]';
  if(bare==='includes')key=family==='chapter'||family==='chapterView'?'includes':'includes[]';
  if(family==='manga'&&key==='translatedLanguage[]')key='availableTranslatedLanguage[]';
  append(key,value);
 }
 if(yearFrom!==undefined||yearTo!==undefined){if(yearFrom===undefined||yearTo===undefined||yearFrom!==yearTo||out.has('year'))fail('Year ranges have no verified public contract. The filter was not dropped.');append('year',yearFrom);}
 if(collection&&!out.has('limit'))append('limit','100');
 if(page!==undefined){if(!collection||!/^\d+$/.test(page)||Number(page)<1||out.has('offset'))fail('Invalid or ambiguous page/offset');append('offset',String((Number(page)-1)*Number(out.get('limit'))));}
 if(sort!==undefined){const field=sortMap[sort]??sort;append('order['+field+']',direction??'desc');}else if(direction!==undefined){if(!['chapter','feed','chapterView'].includes(family))fail('Order requires a known sort');append('order[chapter]',direction);}
 const includeKey=family==='chapter'||family==='chapterView'?'includes':'includes[]';for(const include of includes)if(!out.getAll(includeKey).includes(include))append(includeKey,include);
 return out;
}
export function canUseMangaFeed(query:URLSearchParams):boolean{return [...query.keys()].every(key=>key==='limit'||key==='offset'||key.startsWith('order[')||feedScalar.includes(key)||feedArrays.includes(key)||key==='includes');}
