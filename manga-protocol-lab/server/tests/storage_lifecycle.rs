//! Application-used filesystem lifecycle, with a real Linux file lease as the
//! external I/O fault. No private blocking closures or semaphore inspection.
use anyhow::{Context, Result, ensure};
use manga_node::storage::FilesystemVolume;
use sha2::{Digest, Sha256};
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

async fn cancellation_preserves_owned_io(read: bool) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    let volume = Arc::new(FilesystemVolume::open(root.clone())?);
    let data = b"real filesystem lease blocks an application storage operation";
    let hash: [u8; 32] = Sha256::digest(data).into();
    let receipt = volume
        .mutation(hash)
        .await
        .put_verified([7; 16], data, 1024)
        .await?;
    let key = receipt.key().clone();
    let encoded = hex::encode(hash);
    // External fault arrangement against the persisted object, not an outcome oracle.
    let path = root
        .join("objects")
        .join(&encoded[..2])
        .join(&encoded[2..4])
        .join(format!("{encoded}.{}", hex::encode(key.generation)));
    let mut lease = tokio::process::Command::new("python3")
        .args([
            "-u",
            "-c",
            r#"
import fcntl, signal, sys
signal.signal(signal.SIGIO, lambda *_: print('BREAK', flush=True))
with open(sys.argv[1], 'r+b') as f:
    fcntl.fcntl(f, fcntl.F_SETLEASE, fcntl.F_WRLCK)
    print('READY', flush=True)
    sys.stdin.readline()
    fcntl.fcntl(f, fcntl.F_SETLEASE, fcntl.F_UNLCK)
"#,
        ])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut output = BufReader::new(lease.stdout.take().context("lease stdout")?).lines();
    ensure!(
        tokio::time::timeout(Duration::from_secs(5), output.next_line())
            .await??
            .as_deref()
            == Some("READY"),
        "external lease unavailable"
    );
    let worker = volume.clone();
    let target = key.clone();
    let task = tokio::spawn(async move {
        if read {
            worker
                .read_verified(&target, data.len() as u64, 1024)
                .await
                .map(|_| ())
        } else {
            worker
                .mutation(target.hash)
                .await
                .recover(target.generation, data.len() as u64, 1024)
                .await
                .map(|_| ())
        }
    });
    ensure!(
        tokio::time::timeout(Duration::from_secs(5), output.next_line())
            .await??
            .as_deref()
            == Some("BREAK"),
        "public operation never reached blocked filesystem open"
    );
    task.abort();
    ensure!(
        task.await.unwrap_err().is_cancelled(),
        "caller cancellation failed"
    );
    ensure!(
        tokio::time::timeout(Duration::from_millis(100), volume.mutation(hash))
            .await
            .is_err(),
        "cancelled caller released ownership while real I/O remained blocked"
    );
    let other = b"unrelated hash can progress";
    let other_hash: [u8; 32] = Sha256::digest(other).into();
    let other_receipt = tokio::time::timeout(Duration::from_secs(2), async {
        volume
            .mutation(other_hash)
            .await
            .put_verified([8; 16], other, 1024)
            .await
    })
    .await??;
    assert_eq!(
        volume
            .read_verified(other_receipt.key(), other_receipt.size(), 1024)
            .await?,
        other
    );
    drop(volume);
    ensure!(
        FilesystemVolume::open(root.clone()).is_err(),
        "in-flight operation released cross-process volume ownership"
    );
    lease
        .stdin
        .take()
        .context("lease stdin")?
        .write_all(b"\n")
        .await?;
    ensure!(
        tokio::time::timeout(Duration::from_secs(5), lease.wait())
            .await??
            .success(),
        "external lease holder failed"
    );
    let reopened = reopen(&root).await?;
    assert_eq!(
        reopened
            .read_verified(&key, data.len() as u64, 1024)
            .await?,
        data
    );
    reopened
        .mutation(hash)
        .await
        .cleanup(key.generation)
        .await?;
    ensure!(
        reopened
            .read_verified(&key, data.len() as u64, 1024)
            .await
            .is_err(),
        "completed cleanup must make that generation unreadable"
    );
    Ok(())
}
async fn reopen(root: &Path) -> Result<FilesystemVolume> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(volume) = FilesystemVolume::open(root.into()) {
                return volume;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("completed I/O did not release volume ownership")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_public_read_keeps_real_io_and_volume_fences() -> Result<()> {
    cancellation_preserves_owned_io(true).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_public_recovery_keeps_real_io_and_volume_fences() -> Result<()> {
    cancellation_preserves_owned_io(false).await
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn object_path(root: &Path, key: &manga_node::storage::BlobKey) -> std::path::PathBuf {
    let hash = hex::encode(key.hash);
    root.join("objects")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(format!("{hash}.{}", hex::encode(key.generation)))
}

#[tokio::test]
async fn receipt_requires_verified_exact_bytes_and_recovers_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let volume = FilesystemVolume::open(dir.path().into()).unwrap();
    let data = b"original bytes";
    let h = hash(data);
    let m = volume.mutation(h).await;
    assert!(m.put_verified([1; 16], b"different", 100).await.is_err());
    let receipt = m.put_verified([1; 16], data, 100).await.unwrap();
    assert_eq!(receipt.size(), data.len() as u64);
    let again = m.put_verified([1; 16], data, 100).await.unwrap();
    assert_eq!(again.key(), receipt.key());
    drop(m);
    assert_eq!(
        volume
            .read_verified(receipt.key(), receipt.size(), 100)
            .await
            .unwrap(),
        data
    );
}

#[tokio::test]
async fn stale_generation_cleanup_preserves_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let volume = FilesystemVolume::open(dir.path().into()).unwrap();
    let data = b"same hash, distinct storage generations";
    let m = volume.mutation(hash(data)).await;
    let old = m.put_verified([1; 16], data, 100).await.unwrap();
    let new = m.put_verified([2; 16], data, 100).await.unwrap();
    assert_eq!(m.cleanup([1; 16]).await.unwrap().key(), old.key());
    drop(m);
    assert!(
        volume
            .read_verified(old.key(), old.size(), 100)
            .await
            .is_err()
    );
    assert_eq!(
        volume
            .read_verified(new.key(), new.size(), 100)
            .await
            .unwrap(),
        data
    );
    volume
        .mutation(hash(data))
        .await
        .cleanup([1; 16])
        .await
        .unwrap();
}

#[tokio::test]
async fn corruption_and_oversize_do_not_mint_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let volume = FilesystemVolume::open(dir.path().into()).unwrap();
    let data = b"correct";
    let m = volume.mutation(hash(data)).await;
    assert!(m.put_verified([1; 16], data, 2).await.is_err());
    let receipt = m.put_verified([1; 16], data, 100).await.unwrap();
    tokio::fs::write(object_path(dir.path(), receipt.key()), b"corrupt")
        .await
        .unwrap();
    assert!(m.recover([1; 16], data.len() as u64, 100).await.is_err());
    assert!(m.put_verified([1; 16], data, 100).await.is_err());
    m.cleanup([1; 16]).await.unwrap();
}

#[tokio::test]
async fn hash_mutations_serialize_without_blocking_other_hashes() {
    let dir = tempfile::tempdir().unwrap();
    let volume = FilesystemVolume::open(dir.path().into()).unwrap();
    let guard = volume.mutation([1; 32]).await;
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(200),
            volume.mutation([1; 32])
        )
        .await
        .is_err()
    );
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(200),
            volume.mutation([2; 32])
        )
        .await
        .is_ok()
    );
    drop(guard);
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(200),
            volume.mutation([1; 32])
        )
        .await
        .is_ok()
    );
}

#[test]
fn volume_has_one_mutation_owner() {
    let dir = tempfile::tempdir().unwrap();
    let first = FilesystemVolume::open(dir.path().into()).unwrap();
    assert!(FilesystemVolume::open(dir.path().into()).is_err());
    drop(first);
    assert!(FilesystemVolume::open(dir.path().into()).is_ok());
}
