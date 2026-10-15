//! Bounded derived aliases only. RECONSTRUCTED UNVERIFIED after reset.
use crate::{error::{ApiError,ApiResult},state::AppState};
use manga_node::db::Id;
#[derive(Clone,Debug,PartialEq,Eq)]struct Cursor{after:Option<String>,active:Option<String>,head:Option<String>,offset:i64}
fn cursor(row:&tokio_postgres::Row)->Cursor{Cursor{after:row.get(0),active:row.get(1),head:row.get(2),offset:row.get(3)}}
fn id(value:&str)->ApiResult<Id>{if !manga_node::auth::valid_hash(value){return Err(ApiError::unavailable());}hex::decode(value).ok().and_then(|v|v.try_into().ok()).ok_or_else(ApiError::unavailable)}
#[derive(Debug,Default)]pub struct ListAliasImport{pub aliases:usize,pub completed_owner:bool,pub completed_sweep:bool,pub deferred:bool}
pub async fn import_page(state:&AppState)->ApiResult<ListAliasImport>{
 let runtime=state.protocol.publication_runtime().await?;
 let observed={let c=state.db.client().await?;cursor(&c.query_one("SELECT after_publisher,active_publisher,active_head,next_offset FROM platform.protocol_public_list_alias_cursor WHERE singleton",&[]).await?)};
 let owner=if let Some(owner)=&observed.active{Some(id(owner)?)}else{runtime.library_objects.next_public_library_owner(observed.after.as_deref().map(id).transpose()?).await.map_err(|_|ApiError::unavailable())?};
 let source=if let Some(owner)=owner{Some(runtime.library_objects.public_list_alias_page(owner,observed.head.as_deref(),u64::try_from(observed.offset).map_err(|_|ApiError::unavailable())?).await)}else{None};
 let mut client=state.db.client().await?;let tx=client.transaction().await?;
 tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'").await?;
 let actual=cursor(&tx.query_one("SELECT after_publisher,active_publisher,active_head,next_offset FROM platform.protocol_public_list_alias_cursor WHERE singleton FOR UPDATE",&[]).await?);
 if observed!=actual{return Err(ApiError::conflict("Another public-library alias importer advanced the cursor"));}
 let mut report=ListAliasImport::default();
 match source{
  None=>{tx.execute("UPDATE platform.protocol_public_list_alias_cursor SET after_publisher=NULL,active_publisher=NULL,active_head=NULL,next_offset=0 WHERE singleton",&[]).await?;report.completed_sweep=true;}
  Some(Err(_))=>{let owner=hex::encode(owner.unwrap());tx.execute("UPDATE platform.protocol_public_list_alias_cursor SET after_publisher=$1,active_publisher=NULL,active_head=NULL,next_offset=0 WHERE singleton",&[&owner]).await?;report.deferred=true;}
  Some(Ok(page))=>{let owner=hex::encode(page.publisher);if let Some(head)=page.head{
   let lists=page.lists.iter().map(hex::encode).collect::<Vec<_>>();
   tx.query_one("SELECT platform.public_register_protocol_publisher($1)",&[&owner]).await?;
   tx.query("SELECT platform.public_register_protocol_list($1,list_id) FROM unnest($2::text[]) list_id",&[&owner,&lists]).await?;
   report.aliases=lists.len();let completed=page.next_offset.is_none();
   tx.execute("INSERT INTO platform.protocol_public_list_alias_projection(publisher,head,completed) VALUES($1,$2,$3) ON CONFLICT(publisher) DO UPDATE SET head=EXCLUDED.head,completed=CASE WHEN platform.protocol_public_list_alias_projection.head=EXCLUDED.head THEN platform.protocol_public_list_alias_projection.completed OR EXCLUDED.completed ELSE EXCLUDED.completed END,updated_at=clock_timestamp()",&[&owner,&head,&completed]).await?;
   if let Some(offset)=page.next_offset{let offset=i64::try_from(offset).map_err(|_|ApiError::unavailable())?;tx.execute("UPDATE platform.protocol_public_list_alias_cursor SET active_publisher=$1,active_head=$2,next_offset=$3 WHERE singleton",&[&owner,&head,&offset]).await?;}else{tx.execute("UPDATE platform.protocol_public_list_alias_cursor SET after_publisher=$1,active_publisher=NULL,active_head=NULL,next_offset=0 WHERE singleton",&[&owner]).await?;report.completed_owner=true;}
  }else{tx.execute("UPDATE platform.protocol_public_list_alias_cursor SET after_publisher=$1,active_publisher=NULL,active_head=NULL,next_offset=0 WHERE singleton",&[&owner]).await?;report.completed_owner=true;}}
 }
 tx.commit().await?;Ok(report)
}
