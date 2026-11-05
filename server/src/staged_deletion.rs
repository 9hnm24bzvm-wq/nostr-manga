//! Direct deletion of unretained staging. Never a publication moderation bypass.
use crate::{
    db::{Id, Repository},
    publisher_authority::{PublisherPermit, check_publisher_tx},
    storage::BlobKey,
};
use anyhow::{Context, Result, ensure};
#[derive(Clone)]
pub enum DeleteAuthority {
    Publisher(PublisherPermit),
    Group { group: Id, member: Id, version: i64 },
}
#[derive(Debug, thiserror::Error)]
pub enum DeleteDenied {
    #[error("staged object not found")]
    NotFound,
    #[error("current upload grant required")]
    Unauthorized,
    #[error("original is retained or another interest is active")]
    Protected,
}
impl Repository {
    /// Caller holds the filesystem mutation guard until physical receipt/refund.
    pub(crate) async fn claim_staged_deletion(
        &self,
        authority: &DeleteAuthority,
        hash: Id,
        owner: Id,
        volume: &str,
    ) -> Result<BlobKey> {
        let mut c = self.pool.get().await?;
        let tx = c.transaction().await?;
        match authority {
            DeleteAuthority::Publisher(p) => check_publisher_tx(&tx, p)
                .await
                .map_err(|_| DeleteDenied::Unauthorized)?,
            DeleteAuthority::Group {
                group,
                member,
                version,
            } => crate::groups::check_group_tx(&tx, *group, *member, *version, "remove")
                .await
                .map_err(|_| DeleteDenied::Unauthorized)?,
        }
        let row = tx
            .query_opt(
                "SELECT generation,owner,volume,state FROM blob_intents WHERE hash=$1 FOR UPDATE",
                &[&hash.as_slice()],
            )
            .await?
            .ok_or(DeleteDenied::NotFound)?;
        let generation: Vec<u8> = row.get(0);
        ensure!(
            row.get::<_, Vec<u8>>(1) == owner
                && row.get::<_, String>(2) == volume
                && row.get::<_, String>(3) == "available",
            DeleteDenied::Protected
        );
        let local: Option<Vec<u8>> = match authority {
            DeleteAuthority::Publisher(p) => Some(p.publisher.to_vec()),
            _ => None,
        };
        let group: Option<Vec<u8>> = match authority {
            DeleteAuthority::Group { group, .. } => Some(group.to_vec()),
            _ => None,
        };
        match authority {
            DeleteAuthority::Publisher(p) if p.publisher != owner => {
                ensure!(tx.query_one("SELECT EXISTS(SELECT 1 FROM publisher_blob_grants WHERE publisher=$1 AND hash=$2 AND generation=$3 AND permission_version=$4)", &[&p.publisher.as_slice(),&hash.as_slice(),&generation,&p.version]).await?.get::<_,bool>(0),DeleteDenied::Unauthorized);
            }
            DeleteAuthority::Group { group, .. } => {
                ensure!(tx.query_one("SELECT EXISTS(SELECT 1 FROM group_blob_grants WHERE group_key=$1 AND hash=$2 AND generation=$3)", &[&group.as_slice(),&hash.as_slice(),&generation]).await?.get::<_,bool>(0),DeleteDenied::Unauthorized);
            }
            _ => {}
        }
        // Retained objects (including retirement grace) belong to the publication
        // or library lifecycle. Only never-retained staging is eligible here.
        let unretained:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM blob_retention_credits WHERE hash=$1 AND generation=$2 AND NOT staging_released)", &[&hash.as_slice(),&generation]).await?.get(0);
        ensure!(unretained, DeleteDenied::Protected);
        let protected:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM publication_blobs WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM library_object_holds WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM public_export_holds WHERE hash=$1 AND generation=$2) OR EXISTS(SELECT 1 FROM account_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp()) OR EXISTS(SELECT 1 FROM publisher_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp() AND ($3::bytea IS NULL OR publisher<>$3)) OR EXISTS(SELECT 1 FROM group_blob_grants WHERE hash=$1 AND generation=$2 AND lease_until>clock_timestamp() AND ($4::bytea IS NULL OR group_key<>$4)) OR EXISTS(SELECT 1 FROM staging_operation_blobs h JOIN staging_operations o ON o.id=h.operation WHERE h.hash=$1 AND h.generation=$2 AND o.expires_at>clock_timestamp()) OR EXISTS(SELECT 1 FROM review_blob_grants g JOIN review_submissions s ON s.event_id=g.submission WHERE g.hash=$1 AND g.generation=$2 AND s.state='pending' AND s.expires_at>clock_timestamp())", &[&hash.as_slice(),&generation,&local,&group]).await?.get(0);
        ensure!(!protected, DeleteDenied::Protected);
        if let Some(publisher) = local {
            tx.execute("UPDATE publisher_blob_grants SET lease_until=clock_timestamp() WHERE publisher=$1 AND hash=$2 AND generation=$3", &[&publisher,&hash.as_slice(),&generation]).await?;
        }
        if let Some(group) = group {
            tx.execute("UPDATE group_blob_grants SET lease_until=clock_timestamp() WHERE group_key=$1 AND hash=$2 AND generation=$3", &[&group,&hash.as_slice(),&generation]).await?;
        }
        tx.execute(
            "UPDATE blob_intents SET staging_until=clock_timestamp() WHERE hash=$1",
            &[&hash.as_slice()],
        )
        .await?;
        tx.execute("INSERT INTO blob_gc_queue(hash,generation,due_at,source_mask) VALUES($1,$2,clock_timestamp(),4) ON CONFLICT(hash,generation) DO UPDATE SET due_at=LEAST(blob_gc_queue.due_at,EXCLUDED.due_at),source_mask=blob_gc_queue.source_mask|4", &[&hash.as_slice(),&generation]).await?;
        // Atomic claim prevents an upload that completed I/O earlier from adding
        // a fresh grant in the gap before physical cleanup begins.
        tx.execute("INSERT INTO blob_reclaims(hash,generation) VALUES($1,$2) ON CONFLICT(hash,generation) DO NOTHING", &[&hash.as_slice(),&generation]).await?;
        let key = BlobKey {
            hash,
            generation: generation
                .try_into()
                .ok()
                .context("stored generation encoding")?,
        };
        tx.commit().await?;
        Ok(key)
    }
}
