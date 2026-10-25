// Actual cover controller and signed transport; finite HTTP/signature/image
// stand-ins only. The real backend/DB contract has a separate owned fixture.
import { fixture, row, json, entity, manga, actor, metadata, plain } from './cover-command-fixture.mjs';
export * from './cover-command-fixture.mjs';
export const coverId=index=>`99999999-9999-8999-8999-${String(index).padStart(12,'0')}`;
export const deferred=()=>{let release;const promise=new Promise(resolve=>release=resolve);return{promise,release};};
export function paginationFixture(count=205){
 let rows=Array.from({length:count},(_,index)=>{const value=row(coverId(index+1));value.attributes.volume=String(index+1);value.attributes.description='Cover '+(index+1);value.attributes.nostr.main=index===count-1;return value;}),hook;
 const f=fixture({handler:async(request)=>{
  const custom=await hook?.(request,rows);if(custom!==undefined)return custom;
  if(request.init.method==='GET'&&request.path.endsWith('/covers')){
   const query=new URL(request.url).searchParams,offset=Number(query.get('offset')??0),limit=Number(query.get('limit')??100);
   return json({result:'ok',response:'collection',data:rows.slice(offset,offset+limit),limit,offset,total:rows.length});
  }
  if(request.init.method==='POST'){
   const value=row(coverId(rows.length+1));for(const old of rows)if(old.attributes.nostr.main){old.attributes.nostr.main=false;old.attributes.version++;}
   rows.unshift(value);return entity(value,201);
  }
  if(['PUT','DELETE'].includes(request.init.method)){
   const current=rows.find(row=>request.path.endsWith('/'+row.id)),patch=JSON.parse(new TextDecoder().decode(request.body));
   if(!current)return json({result:'error'},404);if(current.attributes.version!==patch.version)return json({result:'error'},409);
   if(patch.expectedAccountId!==actor)return json({result:'error'},409);
   if(request.init.method==='DELETE'){rows=rows.filter(row=>row!==current);return new Response(null,{status:204});}
   if(patch.main)for(const value of rows)if(value!==current&&value.attributes.nostr.main){value.attributes.nostr.main=false;value.attributes.version++;}
   current.attributes.version++;
   for(const key of['locale','volume','description'])if(key in patch)current.attributes[key]=patch[key];
   if('main'in patch)current.attributes.nostr.main=patch.main;
   return entity(current);
  }
 }});
 Object.assign(f.title,{language:'ja',state:'published',locked:false});f.title.public_api.resource.relationships=[{type:'creator',id:actor}];
 return{...f,rows:()=>rows,setRows:next=>{rows=plain(next);},hook:value=>{hook=value;},reads:()=>f.calls.filter(row=>row.path.endsWith('/covers')),writes:()=>f.calls.filter(row=>row.init.method!=='GET')};
}
export const changed=(record,description='Changed')=>({locale:record.locale,volume:record.volume,description,main:record.main});
