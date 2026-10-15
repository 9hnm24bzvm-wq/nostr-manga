//! Read-only signed library facade over the exact shared original runtime owner.
use super::ProtocolFacade;
use manga_node::{db::Id,http_auth::AdmittedHttpCapability,library_view::ReadMarkerSnapshot,library_lists::PublicListSnapshot};
impl ProtocolFacade{
 pub async fn public_read_markers(&self,capability:&AdmittedHttpCapability,series:&[Id],expected_head:Option<&str>)->anyhow::Result<ReadMarkerSnapshot>{let runtime=self.publication_runtime().await.map_err(|_|anyhow::anyhow!("original library runtime unavailable"))?;runtime.library_objects.public_read_markers(capability,series,expected_head).await}
 pub async fn verify_read_marker_snapshot(&self,capability:&AdmittedHttpCapability,snapshot:&ReadMarkerSnapshot)->anyhow::Result<()>{let runtime=self.publication_runtime().await.map_err(|_|anyhow::anyhow!("original library runtime unavailable"))?;runtime.library_objects.verify_read_marker_snapshot(capability,snapshot).await}
 pub async fn public_lists(&self,publisher:Id,expected:Option<&str>,offset:u64,limit:usize,target:Option<[u8;16]>)->anyhow::Result<PublicListSnapshot>{let runtime=self.publication_runtime().await.map_err(|_|anyhow::anyhow!("original library runtime unavailable"))?;runtime.library_objects.public_lists(publisher,expected,offset,limit,target).await}
 pub async fn verify_public_list_snapshot(&self,snapshot:&PublicListSnapshot,capability:Option<&AdmittedHttpCapability>)->anyhow::Result<()>{
  let runtime=self.publication_runtime().await.map_err(|_|anyhow::anyhow!("original library runtime unavailable"))?;
  if let Some(capability)=capability{
   anyhow::ensure!(snapshot.publisher==capability.principal(),"library owner mismatch");
   let mut client=self.repository.pool.get().await?;let tx=client.transaction().await?;
   tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'").await?;
   capability.recheck_transaction(&tx).await?;
   runtime.library_objects.verify_public_list_snapshot(snapshot).await?;
   tx.commit().await?;
  }else{runtime.library_objects.verify_public_list_snapshot(snapshot).await?;}
  Ok(())
 }
}
