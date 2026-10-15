// RECONSTRUCTED UNVERIFIED. Merge into existing library_view.rs with its imports.
pub struct ReadMarkerSnapshot{pub head:Option<String>,pub root:Option<crate::db::Id>,pub chapters:Vec<ReadMarkerChapter>}
pub struct ReadMarkerChapter{pub chapter:crate::db::Id,pub series:crate::db::Id}
impl LibraryObjects{
 pub async fn public_read_markers(self:&Arc<Self>,capability:&crate::http_auth::AdmittedHttpCapability,series:&[Id],expected_head:Option<&str>)->Result<ReadMarkerSnapshot>{
  ensure!(!series.is_empty()&&series.len()<=100,ViewBudget("manga_ids"));let _read=crate::library_admission::wait(self.reads.clone()).await?;let publisher=capability.principal();
  let Some(view)=self.public_view(publisher,expected_head).await?else{return Ok(ReadMarkerSnapshot{head:None,root:None,chapters:vec![]});};
  let keys:Vec<Vec<u8>>=series.iter().map(|id|id.to_vec()).collect();let mut client=self.repo.pool.get().await?;let tx=client.transaction().await?;
  tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'").await?;capability.recheck_transaction(&tx).await?;
  let parent_visibility=crate::catalogue::VISIBLE.replace("e.","s.");let rows=tx.query(&format!("SELECT e.id,e.series FROM events e WHERE e.kind=1001 AND e.series=ANY($1) AND {} AND EXISTS(SELECT 1 FROM events s WHERE s.id=e.series AND s.kind=1002 AND {}) ORDER BY e.id LIMIT 5001",crate::catalogue::VISIBLE,parent_visibility),&[&keys]).await?;tx.commit().await?;drop(client);
  ensure!(rows.len()<=5000,ViewBudget("manga_chapters"));let ids=rows.iter().map(|r|r.get::<_,Vec<u8>>(0)).collect::<Vec<_>>();let records=self.view_lookup(&view,Family::Chapters,&ids).await?;
  let completed=records.into_iter().filter_map(|r|match r{Record::Chapter{id,completed:true,..}=>Some(id),_=>None}).collect::<std::collections::BTreeSet<_>>();let mut chapters=vec![];
  for row in rows{let chapter:Id=row.get::<_,Vec<u8>>(0).try_into().map_err(|_|anyhow::anyhow!("invalid stored chapter identifier"))?;if completed.contains(&chapter){chapters.push(ReadMarkerChapter{chapter,series:row.get::<_,Vec<u8>>(1).try_into().map_err(|_|anyhow::anyhow!("invalid stored series identifier"))?});}}
  self.verify_view(&view).await?;Ok(ReadMarkerSnapshot{head:Some(view.event.id),root:Some(view.root_hash),chapters})
 }
 // Reconstructed final recheck uses the SAME transaction (no nested checkout).
 // Settings BEFORE authority locks preserve bounded failure under contention.
 pub async fn verify_read_marker_snapshot(&self,capability:&crate::http_auth::AdmittedHttpCapability,snapshot:&ReadMarkerSnapshot)->Result<()>{
  let mut client=self.repo.pool.get().await?;let tx=client.transaction().await?;
  tx.batch_execute("SET LOCAL statement_timeout='2s'; SET LOCAL lock_timeout='500ms'").await?;capability.recheck_transaction(&tx).await?;
  let row=tx.query_opt("SELECT e.id,e.raw_event FROM account_heads h JOIN events e ON e.id=h.event_id AND e.publisher=h.publisher AND e.kind=h.kind WHERE h.publisher=$1 AND h.kind=11999 AND e.state='hosted' AND NOT EXISTS(SELECT 1 FROM publication_blobs p JOIN quarantined_blobs q ON q.hash=p.hash AND q.generation=p.generation WHERE p.event_id=e.id) AND NOT EXISTS(SELECT 1 FROM library_object_holds l JOIN quarantined_blobs q ON q.hash=l.hash AND q.generation=l.generation WHERE l.publisher=e.publisher AND l.state='current')",&[&capability.principal().as_slice()]).await?;
  let current=if let Some(row)=row{let event:crate::auth::Event=serde_json::from_value(row.get(1))?;event.verify().map_err(anyhow::Error::msg)?;ensure!(event.pubkey==hex::encode(capability.principal())&&event.kind==11999&&hex::decode(&event.id)?==row.get::<_,Vec<u8>>(0),"stored library head identity mismatch");Some(event.id)}else{None};
  if current!=snapshot.head{return Err(SnapshotChanged.into());}tx.commit().await?;Ok(())
 }
}
