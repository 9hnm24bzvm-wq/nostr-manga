import { publicUuidLibraryEnabled } from '../public-api/library-staging';
import { publicLibraryCandidates } from '../public-api/library-catalogue';
import { apiRead, assertCatalogueScope, assertCurrentCatalogueScopeValue, captureLibraryContext, assertLibraryContext, currentDeviceSnapshot, isCurrentCatalogueId } from './library';
// Display metadata only: titles never replace saved references or permit rebinding.
export async function loadDeviceListTitles(ids: string[], assertCurrent = () => {}): Promise<Record<string,string>> {
 if(ids.length>100 || new Set(ids).size!==ids.length) throw Error('Choose up to 100 saved titles at a time');
 assertCurrent(); await assertCatalogueScope(); assertCurrent();
 const context=captureLibraryContext(),snapshot=await currentDeviceSnapshot();
 const current=()=>{assertCurrent();assertLibraryContext(context);assertCurrentCatalogueScopeValue(snapshot.scope);}; current();
 if(ids.some(id=>!isCurrentCatalogueId(id)||!snapshot.records.some(row=>row.family==='series'&&row.id===id)))throw Error('The saved title selection changed');
 const query=new URLSearchParams({limit:'100',page:'1',sort:'createdAt'});
 for(const rating of ['safe','suggestive','erotica','pornographic'])query.append('contentRating',rating);
 let rows: {id:string;title:string}[];
 if(publicUuidLibraryEnabled) rows=await publicLibraryCandidates('/manga','id',ids,query);
 else { for(const id of ids)query.append('id',id);const result=await apiRead<{data:{id:string;title:string}[]}>('/manga',query);rows=result.data; }
 current();
 if(!Array.isArray(rows)||rows.length>ids.length||new Set(rows.map(row=>row.id)).size!==rows.length||rows.some(row=>!ids.includes(row.id)||typeof row.title!=='string'||!row.title.trim()))throw Error('The catalogue could not identify these saved titles');
 return Object.fromEntries(rows.map(row=>[row.id,row.title]));
}
