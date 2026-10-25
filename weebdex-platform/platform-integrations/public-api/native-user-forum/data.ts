// Public projection shared by anonymous reads and signed native-discussion snapshots.
import {record,PublicApiContractError} from '../validation';
export interface PublicReply {id:number;number:number;version:number;body?:string;state:string;createdAt:string;vote?:-1|0|1;author?:{id:string;name:string};up:number;down:number;}
export interface PublicDiscussionPage {total:number;page:number;limit:number;data:PublicReply[];}
function fail(message:string,status=502):never{throw new PublicApiContractError(message,status);}
function nativeId(value:unknown):string{if(typeof value!=='string'||!value||value.length>128||/[\\/\u0000-\u0020]/.test(value))fail('Invalid native discussion identity');return value;}
function count(value:unknown,minimum=0):number{if(!Number.isSafeInteger(value)||Number(value)<minimum)fail('Invalid actual discussion count');return Number(value);}
export function normalizeDiscussionPage(input:unknown,page:number,personal=false):PublicDiscussionPage{
 const body=record(input,'public discussion page'),total=count(body.total),limit=count(body.limit,1);
 if(limit!==20||body.page!==page||!Array.isArray(body.data)||body.data.length>limit||body.data.length>Math.max(0,total-(page-1)*limit))fail('Profile comments escaped their requested page');
 const seen=new Set<number>(),data=body.data.map(value=>{
  const row=record(value,'public reply'),id=count(row.id,1),number=count(row.reply_number,1),version=count(row.version,1),relations=record(row.relationships,'public reply relationships'),stats=record(relations.stats,'public reply counts');
  if(seen.has(id)||!['active','locked','flagged','deleted_by_user','deleted_by_moderator','deleted_by_system'].includes(row.state as string)||typeof row.created_at!=='string'||!Number.isFinite(Date.parse(row.created_at))||row.body!==undefined&&(typeof row.body!=='string'||new TextEncoder().encode(row.body).byteLength>50000)||!personal&&relations.vote_state!==undefined||personal&&relations.vote_state!==undefined&&![-1,0,1].includes(relations.vote_state as number))fail('Invalid public reply');seen.add(id);
  if(String(row.state).startsWith('deleted_')&&(row.body!==undefined||relations.author!==undefined))fail('A removed comment exposed private body or author fields');
  let author:PublicReply['author'];if(relations.author!==undefined){const user=record(relations.author,'public reply author');if(typeof user.name!=='string'||!user.name||user.name.length>256)fail('Invalid public reply author');author={id:nativeId(user.id),name:user.name};}
  return{id,number,version,...(row.body===undefined?{}:{body:row.body as string}),state:row.state as string,createdAt:row.created_at as string,...(author?{author}:{}),up:count(stats.up),down:count(stats.down),...(personal?{vote:(relations.vote_state??0) as -1|0|1}:{})};
 });return{total,page,limit,data};
}
