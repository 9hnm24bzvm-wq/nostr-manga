import type {Resource} from './types';
export interface ChapterEditAccess {canUpdate:boolean;canClaim:boolean;canRemove:boolean;approvesHide:boolean;claimFactsAvailable:boolean;}
// These are read-only hints from exact native projections, never write authority.
// Officer status is meaningful only under the explicit local role descriptor.
export function chapterEditAffordances(chapter:Resource,account:{id:string;roles:string[];permissions:string[]},groups:Resource[]):ChapterEditAccess {
 const staff=account.roles.some(role=>['moderator','staff','sysop'].includes(role)),has=(permission:string)=>account.permissions.includes(permission);
 const uploaders=chapter.relationships.filter(row=>row.type==='user');
 const uploader=uploaders.length===1&&chapter.attributes.uploader===uploaders[0].id?uploaders[0].id:undefined;
 const owner=uploader===account.id,groupIds=new Set(chapter.relationships.filter(row=>row.type==='scanlation_group').map(row=>row.id));
 let leader=false,officer=false;
 for(const group of groups){
  if(group.type!=='scanlation_group'||!groupIds.has(group.id)||group.attributes.localMembershipRoles!=='leader-officer-member-v1')continue;
  const edges=group.relationships.filter(row=>['leader','member'].includes(row.type)&&row.id===account.id);
  if(edges.length!==1)continue;
  leader ||= edges[0].type==='leader';officer ||= edges[0].attributes?.isOfficer===true;
 }
 const claimFactsAvailable=!!uploader&&typeof chapter.attributes.localUploaderIsSystem==='boolean';
 const approvesHide=staff&&has('chapter.remove');
 return {canUpdate:staff&&has('chapter.update')||has('self.chapter.update')&&(owner||leader||officer),
  canRemove:approvesHide||has('self.chapter.remove')&&(owner||leader),approvesHide,
  canClaim:has('self.chapter.update')&&leader&&!!uploader&&!owner&&chapter.attributes.localUploaderIsSystem===false,claimFactsAvailable};
}
