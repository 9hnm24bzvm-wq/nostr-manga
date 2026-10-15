// Add to PublicIdTarget enum (display target ONLY):
// ProtocolList {owner:String,list_id:String}
// Merge into persisted_target(kind,native_id,principal_type):
// "native" guard additionally rejects ResourceKind::List native_id starting protocol-list:
// "protocol_list" if kind==ResourceKind::List => {
//   let(owner,list_id)=native_id.strip_prefix("protocol-list:").and_then(|v|v.split_once(':')).ok_or_else(ApiError::unavailable)?;
//   if !manga_node::auth::valid_hash(owner)||list_id.len()!=32||!list_id.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(ApiError::unavailable());}
//   Ok(PublicIdTarget::ProtocolList{owner:owner.to_owned(),list_id:list_id.to_owned()})
// }
// Test valid exact owner/list; malformed/uppercase/extra component, wrong kind,
// native reserved prefix masquerade and protocol_publisher-on-list all fail.
