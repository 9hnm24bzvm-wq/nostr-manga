// APPEND to original library repository.rs; imports Value/PgStore/ApiResult/ApiError already exist.
pub struct PublicUserProfile {pub username:String,pub roles:Vec<String>,pub version:i32}
pub async fn public_user_profile(db:&PgStore,user:&str)->ApiResult<Option<PublicUserProfile>>{
let row=db.client().await?.query_opt("SELECT username,CASE WHEN octet_length(roles::text)<=32768 THEN roles ELSE NULL END,version FROM platform.users WHERE id=$1 AND NOT disabled",&[&user]).await?;
row.map(|row|{let roles:Value=row.get::<_,Option<Value>>(1).ok_or_else(ApiError::unavailable)?;let roles:Vec<String>=serde_json::from_value(roles).map_err(|_|ApiError::unavailable())?;if roles.len()>128||roles.iter().any(|r|r.is_empty()||r.len()>128){return Err(ApiError::unavailable());}Ok(PublicUserProfile{username:row.get(0),roles,version:row.get(2)})}).transpose()
}
