import { publicWriteLanguage } from './language';
// Typed form state only. Reads use actual resource UUIDs; authority stays in the
// signed controllers and the current server-side native account transaction.
import type { MangaView, Resource, Relationship } from './types';
import { PublicApiClient, type Transport } from './client';
import { publicUuid, record, localized, PublicApiContractError } from './validation';
// Read-only affordances mirror native require_manga_edit. They do not authorize
// writes: the signed endpoint rechecks account grants, row state and version.
// Only the server's creator relationship identifies a draft owner; a title UUID
// or a creator display name is never an account ownership proof.
export function titleEditAffordances(current:MangaView|undefined,account:{id:string;roles:string[];permissions:string[]}) {
 const staff=account.roles.some(role=>['moderator','staff','sysop'].includes(role));
 const has=(permission:string)=>account.permissions.includes(permission);
 const creators=current?.public_api.resource.relationships.filter(row=>row.type==='creator')??[];
 const owner=creators.length===1&&creators[0].id===account.id;
 const allowed=(permission:string)=>!current?has('manga.create'):
  staff&&has(permission)||!current.locked&&(current.state==='published'?has(permission):owner&&has('manga.create'));
 const write=allowed('manga.update');
 return {write,title:write&&allowed('manga.title.edit'),authors:write&&allowed('manga.author.edit'),artists:write&&allowed('manga.artist.edit'),tags:write&&allowed('manga.tag.edit'),links:write&&allowed('manga.link.edit'),
  submit:!!current&&['draft','rejected'].includes(current.state)&&has('manga.create')&&(owner||staff)&&(!current.locked||staff),
  review:staff&&has('manga.state.update')};
}
export type EditorChoice={id:string;name:string};
export type LocalizedField={language:string;text:string};
export const titleLinks=[['al','AniList'],['ap','Anime-Planet'],['mu','MangaUpdates'],['nu','Novel Updates'],['kt','Kitsu'],['mal','MyAnimeList'],['bw','Book Walker'],['amz','Amazon'],['ebj','eBookJapan'],['cdj','CDJapan'],['raw','Official Raw URL'],['engtl','Official English URL']] as const;
export const authorLinks=[['website','Website'],['twitter','Twitter'],['pixiv','Pixiv'],['melonBook','Melonbooks'],['fanBox','FANBOX'],['booth','Booth'],['nicoVideo','Nico Video'],['skeb','Skeb'],['fantia','Fantia'],['tumblr','Tumblr'],['youtube','YouTube'],['weibo','Weibo'],['naver','Naver']] as const;
export const groupLinks=[['website','Website'],['twitter','Twitter'],['discord','Discord invite'],['contactEmail','Contact email'],['mangaUpdates','MangaUpdates'],['ircServer','IRC server'],['ircChannel','IRC channel']] as const;
export function choice(row:Relationship):EditorChoice {const a=row.attributes??{};return{id:publicUuid(row.id),name:typeof a.name==='string'?a.name:typeof a.username==='string'?a.username:a.name&&typeof a.name==='object'?localized(a.name):row.id};}
export function choices(rows:Relationship[],kind:string):EditorChoice[]{return rows.filter(r=>r.type===kind).map(choice);}
export function localizedRows(value:Record<string,string[]>):LocalizedField[]{return Object.entries(value).flatMap(([language,rows])=>rows.map(text=>({language,text})));}
export function localizedMap(rows:LocalizedField[]):Record<string,string[]> {const result:Record<string,string[]>=Object.create(null);for(const row of rows){if(!row.language.trim()||!row.text.trim())throw Error('Complete or remove each alternate title.');(result[row.language]??=[]).push(row.text);}return result;}
const same=(a:unknown,b:unknown)=>JSON.stringify(a)===JSON.stringify(b);
// Mirrors the retained ExternalLink provider URL rules without importing its
// native API barrel. Preserve untouched stored values, including unknown keys.
const titleLinkRules:Record<string,{url:RegExp;opaqueId?:boolean;extract?:(match:RegExpMatchArray)=>string}>={
 al:{url:/^https?:\/\/(?:[\w-]+\.)?anilist\.co\/manga\/([0-9]+)(?:\/|$|[?#])/,opaqueId:true,extract:m=>m[1]},
 ap:{url:/^https?:\/\/(?:[\w-]+\.)?anime-planet\.com\/manga\/([a-zA-Z0-9-]+)(?:\/|$|[?#])/,opaqueId:true,extract:m=>m[1]},
 mu:{url:/^https?:\/\/(?:[\w-]+\.)?mangaupdates\.com\/(?:series\/([a-zA-Z0-9-]+)|series\.html\?id=([a-zA-Z0-9-]+))/,opaqueId:true,extract:m=>m[1]??m[2]},
 nu:{url:/^https?:\/\/(?:[\w-]+\.)?novelupdates\.com\/series\/([a-zA-Z0-9-]+)(?:\/|$|[?#])/,opaqueId:true,extract:m=>m[1]},
 kt:{url:/^https?:\/\/(?:[\w-]+\.)?kitsu\.(?:io|app)\/manga\/([a-zA-Z0-9-]+)(?:\/|$|[?#])/,opaqueId:true,extract:m=>m[1]},
 mal:{url:/^https?:\/\/(?:[\w-]+\.)?myanimelist\.net\/manga\/([0-9]+)(?:\/|$|[?#])/,opaqueId:true,extract:m=>m[1]},
 bw:{url:/^https?:\/\/(?:[\w-]+\.)?bookwalker\.jp\/(.+)/,opaqueId:true,extract:m=>m[1]},
 amz:{url:/^https?:\/\/(?:[\w-]+\.)?amazon\.(?:com|co\.jp)\//},
 ebj:{url:/^https?:\/\/ebookjapan\.yahoo\.co\.jp\//},
 cdj:{url:/^https?:\/\/(?:[\w-]+\.)?cdjapan\.co\.jp\//},
 raw:{url:/^https?:\/\//},engtl:{url:/^https?:\/\//}
};
export function normalizeTitleLink(key:string,input:string):string {
 const value=input.trim(),rule=titleLinkRules[key];if(!rule||!value)return value;
 if(/^https?:\/\//.test(value)){let url:URL;try{url=new URL(value);}catch{throw Error('Enter a full HTTP(S) URL for '+key+'.');}if(url.username||url.password)throw Error('Enter a safe HTTP(S) URL for '+key+'.');const match=value.match(rule.url);if(!match)throw Error('The '+key+' URL must use its actual provider.');return rule.extract?rule.extract(match):value;}
 if(!rule.opaqueId||value.startsWith('http'))throw Error('Enter a valid provider ID or full HTTP(S) URL for '+key+'.');return value;
}
// Pinned officialLinks is an independent string dictionary. Do not normalize
// opaque values as legacy provider IDs/URLs or invent language/region records.
export type OfficialLinkField={key:string;value:string};
export function officialLinkDictionary(value:unknown):Record<string,string>{
 const entries=Object.entries(record(value,'officialLinks')),encoder=new TextEncoder();
 if(entries.length>100)throw Error('Official links allow at most 100 entries.');
 for(const[key,text]of entries)if(typeof text!=='string'||encoder.encode(key).byteLength>100||encoder.encode(text).byteLength>2048||/[\u0000]/.test(key+text))throw Error('Official links require string keys/values within the 100/2048-byte text budgets.');
 return Object.fromEntries(entries) as Record<string,string>;
}
export function officialLinkPayload(rows:OfficialLinkField[]):Record<string,string>{
 if(new Set(rows.map(row=>row.key)).size!==rows.length)throw Error('Official link keys must be unique.');
 return officialLinkDictionary(Object.fromEntries(rows.map(row=>[row.key,row.value])));
}
export interface RichTitleFields {altTitles:LocalizedField[];authors:EditorChoice[];artists:EditorChoice[];tags:EditorChoice[];year:string;demographic:string;lastVolume:string;lastChapter:string;links:Record<string,string>;officialLinks:OfficialLinkField[];resetChapters:boolean;}
export function richTitleFields(value?:MangaView):RichTitleFields {return{altTitles:localizedRows(value?.alt_titles??{}),authors:choices(value?.public_api.resource.relationships??[],'author'),artists:choices(value?.public_api.resource.relationships??[],'artist'),tags:(value?.relationships.tags??[]).map(r=>({id:r.id,name:r.name})),year:value?.year?.toString()??'',demographic:value?.demographic??'none',lastVolume:value?.last_volume??'',lastChapter:value?.last_chapter??'',links:{...value?.relationships.links},officialLinks:Object.entries(officialLinkDictionary(value?.public_api.resource.attributes.officialLinks??{})).map(([key,value])=>({key,value})),resetChapters:value?.public_api.resource.attributes.chapterNumbersResetOnNewVolume===true};}
export function titleFieldsChanged(fields:RichTitleFields,value?:MangaView):boolean{return !same(fields,richTitleFields(value));}
export function richTitlePayload(fields:RichTitleFields,value?:MangaView):Record<string,unknown> {
 const before=richTitleFields(value),out:Record<string,unknown>={};
 if(!same(fields.officialLinks,before.officialLinks))out.officialLinks=officialLinkPayload(fields.officialLinks);
 if(!value||!same(fields.altTitles,before.altTitles))out.alt_titles=localizedMap(fields.altTitles);
 for(const key of ['authors','artists','tags']as const){const ids=fields[key].map(row=>publicUuid(row.id));if(new Set(ids).size!==ids.length||ids.length>100)throw Error('Select at most 100 unique '+key+'.');if(!value||!same(ids,before[key].map(r=>r.id)))out[key]=ids;}
 if(!value||fields.year!==before.year){if(fields.year!==''&&!/^[1-9]\d{0,3}$/.test(fields.year))throw Error('Publication year must be blank or 1–9999.');out.year=fields.year===''?null:Number(fields.year);}
 for(const[key,wire]of [['demographic','demographic'],['lastVolume','last_volume'],['lastChapter','last_chapter']]as const)if(!value||fields[key]!==before[key])out[wire]=fields[key]||null;
 if(!value||fields.resetChapters!==before.resetChapters)out.chapterNumbersResetOnNewVolume=fields.resetChapters;
 if(!value||!same(fields.links,before.links)){const links={...fields.links};for(const[key]of titleLinks)if(links[key]&&(!value||links[key]!==before.links[key]))links[key]=normalizeTitleLink(key,links[key]);out.links=Object.fromEntries(Object.entries(links).filter(([,v])=>v!==''));}
 return out;
}
export function createEditorSearch(transport:Transport,assertCurrent:()=>void){const api=new PublicApiClient(transport);return async(kind:'author'|'tag'|'user',query:string,signal?:AbortSignal):Promise<EditorChoice[]>=>{assertCurrent();if(query.length>200)throw Error('Search is too long.');const response=await api.collection(kind==='tag'?'/manga/tag':'/'+kind,kind,kind==='tag'?undefined:new URLSearchParams([[kind==='user'?'username':'name',query],['limit','25']]),signal);assertCurrent();if(response.status!==200||!response.body)throw new PublicApiContractError('Catalogue search refused (HTTP '+response.status+')',response.status);const rows=response.body.data.filter(row=>kind!=='user'||Number.isSafeInteger(row.attributes.version)&&Number(row.attributes.version)>0).map(choice);return kind==='tag'?rows.filter(row=>row.name.toLowerCase().includes(query.toLowerCase())).slice(0,100):rows;};}
export function completeMembershipRoles(row?:Resource):boolean{return row?.attributes.localMembershipRoles==='leader-officer-member-v1'&&row.relationships.filter(r=>r.type==='leader'||r.type==='member').every(r=>typeof r.attributes?.isOfficer==='boolean');}
export interface EditorialFields {name:string;biography:LocalizedField[];description:string;links:Record<string,string>;inactive:boolean;locked:boolean;publishDelay:string;focusedLanguages:string;leader:EditorChoice[];officers:EditorChoice[];members:EditorChoice[];}
export function editorialFields(kind:'author'|'group',row?:Resource):EditorialFields {const a=row?.attributes??{},relations=row?.relationships??[];return{name:String(a.name??''),biography:Object.entries(record(a.biography??{})).map(([language,text])=>({language,text:String(text)})),description:String(a.description??''),links:Object.fromEntries((kind==='author'?authorLinks:groupLinks).map(([key])=>[key,typeof a[key]==='string'?a[key] as string:''])),inactive:a.inactive===true,locked:a.locked===true,publishDelay:typeof a.publishDelay==='string'?a.publishDelay:'',focusedLanguages:Array.isArray(a.focusedLanguage)?a.focusedLanguage.join(', '):'',leader:choices(relations,'leader'),officers:relations.filter(r=>(r.type==='member'||r.type==='leader')&&r.attributes?.isOfficer===true).map(choice),members:relations.filter(r=>r.type==='member'&&r.attributes?.isOfficer!==true).map(choice)};}
export function editorialFieldsChanged(kind:'author'|'group',fields:EditorialFields,row?:Resource){return !same(fields,editorialFields(kind,row));}
export function editorialPayload(kind:'author'|'group',fields:EditorialFields,row?:Resource):Record<string,unknown>{const before=editorialFields(kind,row),out:Record<string,unknown>=row?{version:row.attributes.version}:{};
 const changed=(key:keyof EditorialFields)=>!same(fields[key],before[key]);
 if(!row||changed('name'))out.name=fields.name;
 for(const[key]of kind==='author'?authorLinks:groupLinks)if(fields.links[key]!==before.links[key])out[key]=fields.links[key]||null;
 if(kind==='author'){
  if(changed('biography')){const map:Record<string,string>=Object.create(null);for(const entry of fields.biography){if(!entry.language.trim()||Object.hasOwn(map,entry.language))throw Error('Biography languages must be nonempty and unique.');map[entry.language]=entry.text;}out.biography=map;}
 }else{
  if(!row||changed('description'))out.description=fields.description||null;
  if(changed('inactive'))out.inactive=fields.inactive;
  if(changed('publishDelay'))out.publishDelay=fields.publishDelay;
  if(row&&changed('focusedLanguages'))out.focusedLanguages=fields.focusedLanguages.split(',').map(v=>v.trim()).filter(Boolean).map(publicWriteLanguage);
  if(row&&['leader','officers','members'].some(key=>changed(key as keyof EditorialFields))){
   if(!completeMembershipRoles(row))throw Error('This server has not confirmed complete local officer roles. Reload before editing membership.');
   if(fields.leader.length!==1)throw Error('Select exactly one local group leader.');
   const leader=publicUuid(fields.leader[0].id),officers=fields.officers.map(r=>publicUuid(r.id)),members=fields.members.map(r=>publicUuid(r.id)),all=new Set([leader,...officers,...members]);if(new Set(officers).size!==officers.length||new Set(members).size!==members.length||members.some(id=>id===leader||officers.includes(id))||all.size>100)throw Error('Group roles require distinct members and at most 100 native users. A leader may also be an officer.');
   out.leader=fields.leader[0].id;out.officers=fields.officers.map(r=>r.id);out.members=fields.members.map(r=>r.id);
  }
 }
 if(row&&changed('locked'))out.locked=fields.locked;
 return out;
}
