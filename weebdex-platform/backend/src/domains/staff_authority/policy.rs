//! Reviewed native bundles. These are not the historical fixture permission union.
use crate::{ApiError,ApiResult};
use serde::Serialize;
use std::collections::BTreeSet;
pub const NORMAL: &[&str] = &["self.library.read","self.user.update","self.preferences.update","client.create","self.client.update","self.client.delete","manga.create","author.create","scanlation_group.create","self.scanlation_group.update","chapter.create","self.chapter.update","self.chapter.remove","reply.create","self.reply.update","self.reply.remove","report.create"];
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Bundle {pub id:&'static str,pub label:&'static str,pub roles:&'static [&'static str],pub grants:&'static [&'static str]}
pub const BUNDLES:&[Bundle]=&[
 Bundle{id:"roles-manager",label:"Manage staff access",roles:&["sysop"],grants:&["admin.users.read","admin.roles.manage","admin.audit.read"]},
 Bundle{id:"restriction-manager",label:"Manage local restrictions",roles:&["sysop"],grants:&["admin.users.read","admin.restrictions.manage","admin.audit.read"]},
 Bundle{id:"session-manager",label:"Revoke native sessions",roles:&["sysop"],grants:&["admin.users.read","admin.sessions.revoke","admin.audit.read"]},
 Bundle{id:"security-auditor",label:"Read security history",roles:&["sysop"],grants:&["admin.audit.read"]},
 Bundle{id:"account-reactivator",label:"Reactivate retained accounts",roles:&["sysop"],grants:&["admin.users.read","admin.accounts.manage","admin.audit.read"]},
 Bundle{id:"report-reviewer",label:"Review reports",roles:&["moderator"],grants:&["report.update"]},
 Bundle{id:"report-coordinator",label:"Coordinate report work",roles:&["moderator"],grants:&["report.update","report.assign","report.history"]},
 Bundle{id:"local-moderator",label:"Moderate reported local content",roles:&["moderator"],grants:&["report.update","report.delete","manga.delete","chapter.remove","scanlation_group.update"]},
 Bundle{id:"decision-corrector",label:"Correct local decisions",roles:&["staff","moderator"],grants:&["report.update","report.history","report.correct","manga.restore","chapter.restore","group.leadership.revoke"]},
 Bundle{id:"profile-editor",label:"Edit public profiles",roles:&["staff"],grants:&["user.update"]},
 Bundle{id:"operations-reader",label:"Read instance operations",roles:&["staff"],grants:&["operations.read"]},
 Bundle{id:"announcement-editor",label:"Manage announcements",roles:&["staff"],grants:&["announcement.create","announcement.update","announcement.delete"]},
 Bundle{id:"editorial-reviewer",label:"Review title submissions",roles:&["moderator"],grants:&["manga.state.update"]},
 Bundle{id:"catalogue-editor",label:"Edit catalogue metadata",roles:&["moderator"],grants:&["manga.update","manga.title.edit","manga.tag.edit","manga.author.edit","manga.artist.edit","manga.link.edit","manga.relation.edit","manga.cover.edit","author.update","scanlation_group.update","chapter.update"]},
 Bundle{id:"catalogue-moderator",label:"Moderate catalogue records",roles:&["moderator"],grants:&["manga.lock.update","manga.delete","author.lock.update","author.delete","scanlation_group.delete","cover.delete","chapter.remove"]},
 Bundle{id:"discussion-moderator",label:"Moderate discussion replies",roles:&["moderator"],grants:&["reply.update","reply.remove","reply.delete"]},
];
pub const BOOTSTRAP:&[&str]=&["roles-manager","restriction-manager","session-manager","security-auditor"];
pub fn selection(ids:&[String],verified:bool,restriction_roles:&[String])->ApiResult<(Vec<String>,Vec<String>)>{
 if ids.len()>BUNDLES.len(){return Err(ApiError::bad_request("Choose reviewed staff access bundles"));}
 let mut seen=BTreeSet::new();let mut roles=BTreeSet::from(["user".to_owned()]);let mut grants:BTreeSet<String>=NORMAL.iter().map(|v|(*v).into()).collect();
 if verified{roles.insert("verified".into());}else if !ids.is_empty(){return Err(ApiError::forbidden("Only a verified account may receive staff access"));}
 for id in ids {if !seen.insert(id){return Err(ApiError::bad_request("Choose each access bundle once"));}let bundle=BUNDLES.iter().find(|b|b.id==id).ok_or_else(||ApiError::bad_request("Unknown staff access bundle"))?;roles.extend(bundle.roles.iter().map(|s|(*s).to_owned()));grants.extend(bundle.grants.iter().map(|s|(*s).to_owned()));}
 roles.extend(restriction_roles.iter().filter(|s|matches!(s.as_str(),"banned"|"restricted")).cloned());Ok((roles.into_iter().collect(),grants.into_iter().collect()))
}
#[cfg(test)]mod tests{use super::*;#[test]fn bounded_and_separate(){assert_eq!(NORMAL.len(),17);assert!(selection(&["all".into()],true,&[]).is_err());assert!(selection(&["roles-manager".into()],false,&[]).is_err());let(r,p)=selection(&["roles-manager".into()],true,&[]).unwrap();assert!(r.contains(&"sysop".into()));assert!(!p.contains(&"report.update".into()));assert!(!p.contains(&"announcement.create".into()));let(r,_)=selection(&[],false,&[]).unwrap();assert_eq!(r,vec!["user"]);}}
