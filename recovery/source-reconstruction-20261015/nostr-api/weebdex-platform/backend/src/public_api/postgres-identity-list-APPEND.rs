// RECONSTRUCTED UNVERIFIED. Add ProtocolList{owner:String,list_id:String} to the
// PublicIdTarget enum. In persisted_target, handle principal_type protocol_list
// by calling this helper. Native List keys with protocol-list: prefix must fail.
fn persisted_protocol_list_target(kind:ResourceKind,native_id:&str)->ApiResult<PublicIdTarget>{
 if kind!=ResourceKind::List{return Err(ApiError::unavailable());}
 let(owner,list_id)=native_id.strip_prefix("protocol-list:").and_then(|value|value.split_once(':')).ok_or_else(ApiError::unavailable)?;
 if !manga_node::auth::valid_hash(owner)||list_id.len()!=32||!list_id.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(ApiError::unavailable());}
 Ok(PublicIdTarget::ProtocolList{owner:owner.to_owned(),list_id:list_id.to_owned()})
}
#[cfg(test)]mod protocol_list_recovery_tests{
 use super::*;
 #[test]fn protocol_lists_require_exact_owner_and_list_type(){let owner="a".repeat(64);let list="b".repeat(32);let key=format!("protocol-list:{owner}:{list}");assert_eq!(persisted_protocol_list_target(ResourceKind::List,&key).unwrap(),PublicIdTarget::ProtocolList{owner:owner.clone(),list_id:list.clone()});for malformed in[format!("protocol-list:bad:{list}"),format!("protocol-list:{}:{list}","A".repeat(64)),format!("protocol-list:{owner}:bad"),format!("protocol-list:{owner}:{}","B".repeat(32)),format!("{key}:extra"),format!("{owner}:{list}")]{assert!(persisted_protocol_list_target(ResourceKind::List,&malformed).is_err());}assert!(persisted_protocol_list_target(ResourceKind::User,&key).is_err());
 // Once merged, also assert persisted_target(List,&key,"native").is_err() and
 // persisted_target(List,&key,"protocol_publisher").is_err().
 }
}
