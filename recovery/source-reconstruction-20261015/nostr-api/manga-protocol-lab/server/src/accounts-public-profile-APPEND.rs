// APPEND/merge into original accounts.rs. Preserve the base's existing account
// admission APIs. This is recovered from visible source and needs fresh tests.
use serde_json::{Value,json};
use std::sync::Arc;
pub struct PublicAccountProfile {pub publisher:Id,pub known:bool,pub event:Option<Arc<Event>>,pub metadata:Value}
impl PublicAccountProfile {
    pub fn display_name(&self)->Option<&str>{["display_name","name"].into_iter().find_map(|key|self.metadata.get(key).and_then(Value::as_str).filter(|name|!name.is_empty()&&name.len()<=512))}
}
impl Repository {
    pub async fn public_account_profile(&self,publisher:Id)->Result<PublicAccountProfile>{
        let (known,head):(bool,Option<Vec<u8>>)={let row=self.pool.get().await?.query_one("SELECT EXISTS(SELECT 1 FROM events WHERE publisher=$1 AND state='hosted'),(SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=0)",&[&publisher.as_slice()]).await?;(row.get(0),row.get(1))};
        let event=if let Some(head)=&head{let id:Id=head.as_slice().try_into()?;self.visible_event(id).await?}else{None};
        let metadata=if let Some(event)=&event{
            ensure!(event.kind==0&&event.pubkey==hex::encode(publisher),"stored profile identity mismatch");
            ensure!(event.content.len()<=1048576,"profile metadata byte budget");
            let metadata:Value=serde_json::from_str(&event.content)?;ensure!(metadata.is_object(),"profile metadata object required");metadata
        }else{json!({})};
        let current:Option<Vec<u8>>=self.pool.get().await?.query_one("SELECT (SELECT event_id FROM account_heads WHERE publisher=$1 AND kind=0)",&[&publisher.as_slice()]).await?.get(0);
        ensure!(current==head,"public profile head changed");
        if let Some(event)=&event{ensure!(self.visible_event(hex::decode(&event.id)?.try_into().unwrap()).await?.is_some(),"public profile visibility changed");}
        Ok(PublicAccountProfile{publisher,known,event,metadata})
    }
}
#[cfg(test)]mod public_profile_tests{
use super::*;
#[test]fn display_names_are_bounded_public_metadata_only(){let mut p=PublicAccountProfile{publisher:[1;32],known:true,event:None,metadata:json!({"display_name":"Public display","name":"secondary"})};assert_eq!(p.display_name(),Some("Public display"));p.metadata=json!({"display_name":"","name":"secondary"});assert_eq!(p.display_name(),Some("secondary"));for m in[json!({}),json!({"display_name":false,"name":123}),json!({"display_name":"a".repeat(513),"name":null})]{p.metadata=m;assert!(p.display_name().is_none());}}
}
