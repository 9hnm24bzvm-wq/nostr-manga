//! Requires disposable PostgreSQL. Excluded from protocol-only distributions.
use super::*;
use crate::db::DbConfig;
fn signed(kind: u32, content: &str, timestamp: u64) -> Event {
    crate::auth::sign_at(&"11".repeat(32), kind, vec![], content.into(), timestamp).unwrap()
}
fn config(path: &std::path::Path) -> BackendConfig {
    BackendConfig {
        engine: BackendEngine::Native,
        binary: PathBuf::new(),
        data_dir: path.into(),
        timeout_seconds: 2,
        map_bytes: 16 * 1024 * 1024,
    }
}
async fn import(
    backend: &Backend,
    client: &mut deadpool_postgres::ClientWrapper,
    event: &Event,
) -> Result<()> {
    backend
        .command(
            client,
            &["import".into()],
            serde_json::to_vec(event)?,
            65536,
        )
        .await?;
    Ok(())
}
async fn scan(
    backend: &Backend,
    client: &mut deadpool_postgres::ClientWrapper,
    ids: &[&str],
) -> Result<Vec<Event>> {
    let bytes = backend
        .command(
            client,
            &["scan".into(), json!({"ids":ids}).to_string()],
            vec![],
            2 * 1048576,
        )
        .await?;
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| Ok(serde_json::from_slice(line)?))
        .collect()
}
#[tokio::test]
#[ignore = "requires fresh isolated manga_test PostgreSQL database"]
async fn native_projection_semantics() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("isolated fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    ensure!(
        cfg.get_dbname() == Some("manga_test"),
        "fixture database only"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    repo.verify_schema().await?;
    let control = repo.pool.get().await?;
    let count: i64 = control
        .query_one("SELECT count(*) FROM backend_identity", &[])
        .await?
        .get(0);
    ensure!(count == 0, "fresh database required");
    let count: i64 = control
        .query_one("SELECT count(*) FROM native_backend_events", &[])
        .await?
        .get(0);
    ensure!(count == 0, "empty native projection required");
    let legacy_id = vec![0x5a_u8; 32];
    control
        .execute(
            "INSERT INTO backend_identity(singleton,target_id) VALUES(TRUE,$1)",
            &[&legacy_id],
        )
        .await?;
    let mismatch = tempfile::tempdir()?;
    let error = Backend::open(repo.clone(), config(mismatch.path()))
        .await
        .err()
        .context("legacy identity accepted")?;
    ensure!(
        error.to_string().contains("another backend target"),
        "unexpected identity error: {error}"
    );
    control
        .execute(
            "DELETE FROM backend_identity WHERE target_id=$1",
            &[&legacy_id],
        )
        .await?;
    let root = tempfile::tempdir()?;
    let backend = Backend::open(repo.clone(), config(root.path())).await?;
    let mut client = deadpool_postgres::Object::take(repo.pool.get().await?);
    let locked: bool = client
        .query_one("SELECT pg_try_advisory_lock(697241114)", &[])
        .await?
        .get(0);
    ensure!(locked, "projection lock unavailable");
    let note = signed(1, "regular", 100);
    import(&backend, &mut client, &note).await?;
    import(&backend, &mut client, &note).await?;
    let found = scan(&backend, &mut client, &[&note.id]).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, note.id);
    found[0].verify().map_err(anyhow::Error::msg)?;
    let old = signed(0, "old", 100);
    let a = signed(0, "tie a", 101);
    let b = signed(0, "tie b", 101);
    let (low, high) = if a.id < b.id { (&a, &b) } else { (&b, &a) };
    for event in [&old, high, low, high, &old] {
        import(&backend, &mut client, event).await?;
    }
    let found = scan(&backend, &mut client, &[&old.id, &a.id, &b.id]).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, low.id);
    let raw_delete = signed(5, "not guarded deletion", 102);
    assert!(import(&backend, &mut client, &raw_delete).await.is_err());
    assert!(
        scan(&backend, &mut client, &[&raw_delete.id])
            .await?
            .is_empty()
    );
    backend
        .command(
            &mut client,
            &[
                "delete".into(),
                "--filter".into(),
                json!({"ids":[note.id.clone()]}).to_string(),
            ],
            vec![],
            65536,
        )
        .await?;
    assert!(scan(&backend, &mut client, &[&note.id]).await?.is_empty());
    assert_eq!(scan(&backend, &mut client, &[&low.id]).await?.len(), 1);
    let pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    let killed: bool = control
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .await?
        .get(0);
    assert!(killed);
    let after_loss = signed(1, "must not persist", 103);
    assert!(import(&backend, &mut client, &after_loss).await.is_err());
    let exists: bool = control
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM native_backend_events WHERE id=$1)",
            &[&after_loss.id],
        )
        .await?
        .get(0);
    assert!(!exists);
    Ok(())
}
