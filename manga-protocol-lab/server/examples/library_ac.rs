//! Isolated A/C experiment, NOT a production route or chosen protocol schema.
//! Only runs against the named synthetic fixture database on loopback.
use anyhow::{Context, Result};
macro_rules! check { ($cond:expr, $($arg:tt)*) => { if !$cond { return Err(anyhow::anyhow!($($arg)*).into()); } }; }
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use manga_node::{
    auth::{self, Event},
    db::{DbConfig, Repository},
};
use moka::future::Cache;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashSet},
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Semaphore,
};
#[derive(Clone)]
struct App {
    repo: Repository,
    root: PathBuf,
    io: Arc<Semaphore>,
    writes: Arc<Semaphore>,
    cache: Cache<String, Arc<Vec<u8>>>,
    max_event: usize,
    fault: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Commit {
    event: Event,
    #[serde(default)]
    objects: Vec<Value>,
    #[serde(default)]
    expected: Option<String>,
}
struct ErrResponse(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for ErrResponse {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for ErrResponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, self.0.to_string()).into_response()
    }
}
fn canonical(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn encoded(bytes: Vec<u8>, headers: &HeaderMap) -> Response {
    if headers
        .get("accept-encoding")
        .is_some_and(|v| v.to_str().unwrap_or("").contains("gzip"))
    {
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(3));
        enc.write_all(&bytes).unwrap();
        let body = enc.finish().unwrap();
        (
            [
                ("content-type", "application/json"),
                ("content-encoding", "gzip"),
            ],
            body,
        )
            .into_response()
    } else {
        ([("content-type", "application/json")], bytes).into_response()
    }
}
fn validate_entries(v: &Value) -> Result<()> {
    let entries = v.as_object().context("entries must be object")?;
    check!(entries.len() <= 1_000_000, "too many entries");
    for (id, item) in entries {
        check!(auth::valid_hash(id), "chapter ID");
        check!(item.is_object(), "entry");
    }
    Ok(())
}
async fn commit(
    State(s): State<App>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ErrResponse> {
    let started = Instant::now();
    let _slot = s.writes.acquire().await?;
    let raw = if headers.get("content-encoding").is_some_and(|x| x == "gzip") {
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(&body[..])
            .take(128 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        check!(bytes.len() <= 128 * 1024 * 1024, "inflated body bound");
        bytes
    } else {
        body.to_vec()
    };
    let req: Commit = serde_json::from_slice(&raw)?;
    let e = &req.event;
    e.verify().map_err(anyhow::Error::msg)?;
    check!(e.kind == 11999, "fixture library kind only");
    let event_bytes = serde_json::to_vec(e)?;
    check!(
        event_bytes.len() <= s.max_event,
        "event exceeds configured limit"
    );
    let content: Value = serde_json::from_str(&e.content)?;
    let chunked = content.get("root").is_some();
    let mut objects = Vec::new();
    let mut refs = Vec::<String>::new();
    if chunked {
        check!(req.objects.len() <= 2050, "objects bound");
        for v in &req.objects {
            let bytes = canonical(v);
            check!(bytes.len() <= 16 * 1024 * 1024, "object bound");
            if let Some(x) = v.get("entries") {
                validate_entries(x)?;
            }
            objects.push((auth::hash(&bytes), bytes));
        }
        let h = content["root"].as_str().context("root hash")?;
        check!(auth::valid_hash(h), "invalid root hash");
        let root = if let Some((_, bytes)) = objects.iter().find(|(x, _)| x == h) {
            serde_json::from_slice::<Value>(bytes)?
        } else {
            let bytes = tokio::fs::read(s.root.join(h)).await?;
            check!(auth::hash(&bytes) == h, "root hash mismatch");
            serde_json::from_slice(&bytes)?
        };
        let chunks = root["chunks"].as_array().context("root chunks")?;
        check!(chunks.len() <= 2048, "chunk count");
        refs = chunks
            .iter()
            .map(|x| x.as_str().map(String::from).context("chunk hash"))
            .collect::<Result<_>>()?;
        refs.push(
            root["overview"]
                .as_str()
                .context("overview hash")?
                .to_string(),
        );
        refs.push(h.to_string());
        check!(
            refs.iter().all(|h| auth::valid_hash(h)),
            "invalid reference"
        );
        refs.sort();
        refs.dedup();
    } else {
        check!(req.objects.is_empty(), "A has no external objects");
        validate_entries(&content["entries"])?;
        check!(content["overview"].is_object(), "overview");
    }
    let prepared = started.elapsed().as_secs_f64() * 1000.;
    let io_start = Instant::now();
    let mut newly_written = 0usize;
    if !objects.is_empty() {
        let permit = s.io.clone().acquire_owned().await?;
        let root = s.root.clone();
        let todo = objects.clone();
        newly_written = tokio::task::spawn_blocking(move || -> Result<usize> {
            let _permit = permit;
            let mut wrote = 0;
            for (h, bytes) in todo {
                let dest = root.join(&h);
                if dest.exists() {
                    continue;
                }
                let tmp = root.join(format!(".{}-{}", h, rand::random::<u64>()));
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&tmp)?;
                f.write_all(&bytes)?;
                f.sync_all()?;
                std::fs::rename(&tmp, &dest)?;
                wrote += bytes.len();
            }
            std::fs::File::open(root)?.sync_all()?;
            Ok(wrote)
        })
        .await??;
    }
    if s.fault == "after_objects" {
        std::process::exit(77);
    }
    let io_ms = io_start.elapsed().as_secs_f64() * 1000.;
    let db_start = Instant::now();
    let mut c = s.repo.pool.get().await?;
    // Register verified staged objects separately; visibility is still false.
    if !objects.is_empty() {
        let stage = c.transaction().await?;
        objects.sort_by(|a, b| a.0.cmp(&b.0));
        for (h, bytes) in &objects {
            stage
                .execute(
                    "INSERT INTO ac_objects(hash,size) VALUES($1,$2) ON CONFLICT DO NOTHING",
                    &[h, &(bytes.len() as i64)],
                )
                .await?;
        }
        stage.commit().await?;
    }
    let tx = c.transaction().await?;
    tx.execute(
        "INSERT INTO ac_accounts(pk) VALUES($1) ON CONFLICT DO NOTHING",
        &[&e.pubkey],
    )
    .await?;
    tx.query_one(
        "SELECT pk FROM ac_accounts WHERE pk=$1 FOR UPDATE",
        &[&e.pubkey],
    )
    .await?;
    let old = tx
        .query_opt(
            "SELECT event_id,created_at,refs FROM ac_heads WHERE pk=$1",
            &[&e.pubkey],
        )
        .await?;
    if old.as_ref().is_some_and(|r| r.get::<_, String>(0) == e.id) {
        tx.rollback().await?;
        return Ok(encoded(
            canonical(&json!({"id":e.id,"duplicate":true})),
            &headers,
        ));
    }
    if let Some(expected) = req.expected {
        check!(
            old.as_ref()
                .is_some_and(|r| r.get::<_, String>(0) == expected),
            "CAS conflict"
        );
    }
    if let Some(row) = &old {
        let old_time: i64 = row.get(1);
        let old_id: String = row.get(0);
        check!(
            e.created_at as i64 > old_time || (e.created_at as i64 == old_time && e.id < old_id),
            "superseded snapshot"
        );
    }
    if chunked {
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM ac_objects WHERE hash=ANY($1)",
                &[&refs],
            )
            .await?
            .get(0);
        check!(count as usize == refs.len(), "missing manifest/chunk");
    }
    let old_refs: BTreeSet<String> = old
        .as_ref()
        .map(|r| r.get::<_, Vec<String>>(2).into_iter().collect())
        .unwrap_or_default();
    let new_refs: BTreeSet<String> = refs.iter().cloned().collect();
    let changed: Vec<String> = old_refs.symmetric_difference(&new_refs).cloned().collect();
    if !changed.is_empty() {
        let locked = tx
            .query(
                "SELECT hash FROM ac_objects WHERE hash=ANY($1) ORDER BY hash FOR UPDATE",
                &[&changed],
            )
            .await?;
        check!(locked.len() == changed.len(), "reference object missing");
        for hash in &changed {
            let delta = if new_refs.contains(hash) { 1i64 } else { -1i64 };
            let updated=tx.execute("UPDATE ac_objects SET current_refs=current_refs+$2,visible_until=CASE WHEN $2<0 THEN clock_timestamp()+interval '5 minutes' ELSE visible_until END WHERE hash=$1 AND current_refs+$2>=0", &[hash,&delta]).await?;
            check!(updated == 1, "reference counter mismatch");
        }
    }
    let value = serde_json::to_value(e)?;
    tx.execute("INSERT INTO ac_heads(pk,event_id,created_at,event,refs) VALUES($1,$2,$3,$4,$5) ON CONFLICT(pk) DO UPDATE SET event_id=excluded.event_id,created_at=excluded.created_at,event=excluded.event,refs=excluded.refs",&[&e.pubkey,&e.id,&(e.created_at as i64),&value,&refs]).await?;
    tx.execute(
        "INSERT INTO ac_outbox(event_id,pk,event,refs) VALUES($1,$2,$3,$4)",
        &[&e.id, &e.pubkey, &value, &refs],
    )
    .await?;
    tx.commit().await?;
    if s.fault == "after_commit" {
        std::process::exit(78);
    }
    s.cache
        .insert(format!("event:{}", e.id), Arc::new(event_bytes.clone()))
        .await;
    let response = json!({"id":e.id,"accepted_ms":started.elapsed().as_secs_f64()*1000.,"prepare_ms":prepared,"object_io_ms":io_ms,"db_ms":db_start.elapsed().as_secs_f64()*1000.,"object_bytes_written":newly_written,"event_bytes":event_bytes.len()});
    Ok(encoded(canonical(&response), &headers))
}
async fn head(
    State(s): State<App>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let row = c
        .query_opt("SELECT event_id FROM ac_heads WHERE pk=$1", &[&pk])
        .await?
        .context("missing account")?;
    let id: String = row.get(0);
    let key = format!("event:{id}");
    let bytes = if let Some(b) = s.cache.get(&key).await {
        b
    } else {
        let value: Value = c
            .query_one("SELECT event FROM ac_outbox WHERE event_id=$1", &[&id])
            .await?
            .get(0);
        let b = Arc::new(canonical(&value));
        s.cache.insert(key, b.clone()).await;
        b
    };
    Ok(encoded(bytes.as_ref().clone(), &headers))
}
async fn object(
    State(s): State<App>,
    Path(h): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ErrResponse> {
    check!(auth::valid_hash(&h), "hash");
    let c = s.repo.pool.get().await?;
    // Only objects reachable from a current head are public. Cache never bypasses visibility.
    check!(c.query_opt("SELECT 1 FROM ac_objects WHERE hash=$1 AND (current_refs>0 OR visible_until>clock_timestamp())",&[&h]).await?.is_some(),"object not currently referenced");
    drop(c);
    let bytes = if let Some(x) = s.cache.get(&h).await {
        x
    } else {
        let b = tokio::fs::read(s.root.join(&h)).await?;
        check!(auth::hash(&b) == h, "corrupt object");
        let b = Arc::new(b);
        s.cache.insert(h, b.clone()).await;
        b
    };
    Ok(encoded(bytes.as_ref().clone(), &headers))
}
async fn bundle(
    State(s): State<App>,
    Path(pk): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let row = c
        .query_opt("SELECT event,refs FROM ac_heads WHERE pk=$1", &[&pk])
        .await?
        .context("missing account")?;
    let event: Value = row.get(0);
    let refs: Vec<String> = row.get(1);
    drop(c);
    // One current-head read pins the required immutable set; five-minute history grace protects in-flight reads.
    let mut objects = Vec::with_capacity(refs.len());
    for h in refs {
        let bytes = if let Some(b) = s.cache.get(&h).await {
            b
        } else {
            let b = tokio::fs::read(s.root.join(&h)).await?;
            check!(auth::hash(&b) == h, "corrupt object");
            let b = Arc::new(b);
            s.cache.insert(h, b.clone()).await;
            b
        };
        objects.push(serde_json::from_slice::<Value>(&bytes)?);
    }
    Ok(encoded(
        canonical(&json!({"event":event,"objects":objects})),
        &headers,
    ))
}
async fn stats(State(s): State<App>) -> Result<axum::Json<Value>, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let row=c.query_one("SELECT count(*),count(*) FILTER(WHERE projected IS NULL),coalesce(sum(octet_length(event::text)),0)::bigint FROM ac_outbox",&[]).await?;
    let objects = c
        .query_one(
            "SELECT count(*),coalesce(sum(size),0)::bigint FROM ac_objects",
            &[],
        )
        .await?;
    let db=c.query_one("SELECT pg_current_wal_lsn()::text,pg_database_size(current_database()),current_setting('fsync'),current_setting('synchronous_commit')",&[]).await?;
    let live = c
        .query_one(
            "SELECT coalesce(sum(size),0)::bigint FROM ac_objects WHERE current_refs>0",
            &[],
        )
        .await?
        .get::<_, i64>(0);
    Ok(axum::Json(
        json!({"events":row.get::<_,i64>(0),"pending":row.get::<_,i64>(1),"retained_event_json_bytes":row.get::<_,i64>(2),"objects":objects.get::<_,i64>(0),"retained_object_bytes":objects.get::<_,i64>(1),"live_object_bytes":live,"wal_lsn":db.get::<_,String>(0),"database_bytes":db.get::<_,i64>(1),"fsync":db.get::<_,String>(2),"synchronous_commit":db.get::<_,String>(3)}),
    ))
}
async fn reference_audit(State(s): State<App>) -> Result<axum::Json<Value>, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let errors:i64=c.query_one("WITH expected AS (SELECT h,count(*) AS n FROM ac_heads CROSS JOIN LATERAL unnest(refs) AS h GROUP BY h) SELECT count(*) FROM ac_objects o FULL JOIN expected e ON e.h=o.hash WHERE coalesce(o.current_refs,0)<>coalesce(e.n,0)",&[]).await?.get(0);
    Ok(axum::Json(json!({"reference_count_errors":errors})))
}
async fn health(State(s): State<App>) -> Result<axum::Json<Value>, ErrResponse> {
    let client = s.repo.pool.get().await?;
    let pending: i64 = client
        .query_one(
            "SELECT count(*) FROM ac_outbox WHERE projected IS NULL",
            &[],
        )
        .await?
        .get(0);
    Ok(axum::Json(json!({"pending": pending})))
}
async fn measurements(State(s): State<App>) -> Result<axum::Json<Value>, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let rows=c.query("SELECT event_id,(extract(epoch FROM(projected-accepted))*1000)::double precision FROM ac_outbox ORDER BY id",&[]).await?;
    Ok(axum::Json(Value::Array(
        rows.iter()
            .map(|r| json!({"id":r.get::<_,String>(0),"projection_ms":r.get::<_,Option<f64>>(1)}))
            .collect(),
    )))
}
async fn status(
    State(s): State<App>,
    Path(id): Path<String>,
) -> Result<axum::Json<Value>, ErrResponse> {
    let c = s.repo.pool.get().await?;
    let row=c.query_opt("SELECT projected IS NOT NULL,(extract(epoch FROM(projected-accepted))*1000)::double precision FROM ac_outbox WHERE event_id=$1",&[&id]).await?.context("missing event")?;
    Ok(axum::Json(
        json!({"projected":row.get::<_,bool>(0),"projection_ms":row.get::<_,Option<f64>>(1)}),
    ))
}
async fn command(binary: &str, config: &str, args: &[&str], input: Vec<u8>) -> Result<Vec<u8>> {
    let mut child = tokio::process::Command::new("/usr/bin/timeout")
        .args([
            "--kill-after=2s",
            "30s",
            binary,
            &format!("--config={config}"),
        ])
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdin = child.stdin.take().context("stdin")?;
    let mut stdout = child.stdout.take().context("stdout")?;
    let mut stderr = child.stderr.take().context("stderr")?;
    let (a, b, d, status) = tokio::join!(
        async move {
            stdin.write_all(&input).await?;
            stdin.shutdown().await
        },
        async {
            let mut b = Vec::new();
            stdout.read_to_end(&mut b).await?;
            Ok::<_, std::io::Error>(b)
        },
        async {
            let mut b = Vec::new();
            stderr.read_to_end(&mut b).await?;
            Ok::<_, std::io::Error>(b)
        },
        child.wait()
    );
    a?;
    let out = b?;
    let err = d?;
    check!(
        status?.success(),
        "strfry failed {}",
        String::from_utf8_lossy(&err)
    );
    Ok(out)
}
async fn project(s: App, binary: String, config: String) -> Result<()> {
    // One fixture projector; the full node's process-fencing machinery is not benchmarked here.
    loop {
        let c = s.repo.pool.get().await?;
        let rows=c.query("SELECT id,pk,event_id,event FROM ac_outbox WHERE projected IS NULL ORDER BY id LIMIT 32",&[]).await?;
        if rows.is_empty() {
            drop(c);
            tokio::time::sleep(Duration::from_millis(2)).await;
            continue;
        }
        let mut keys = HashSet::new();
        let mut payload = Vec::new();
        let mut jobs = Vec::new();
        let mut ids = Vec::new();
        for row in rows {
            let pk: String = row.get(1);
            if !keys.insert(pk) {
                break;
            }
            let event: Value = row.get(3);
            let bytes = canonical(&event);
            if !jobs.is_empty() && payload.len() + bytes.len() > 16 * 1024 * 1024 {
                break;
            }
            payload.extend(bytes);
            payload.push(b'\n');
            jobs.push(row.get::<_, i64>(0));
            ids.push(row.get::<_, String>(2));
        }
        command(
            &binary,
            &config,
            &["import", "--debounce-millis=0"],
            payload,
        )
        .await?;
        let filter = json!({"ids":ids}).to_string();
        let out = command(&binary, &config, &["scan", &filter], Vec::new()).await?;
        let found = out
            .split(|x| *x == b'\n')
            .filter(|b| !b.is_empty())
            .map(serde_json::from_slice::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let found: HashSet<_> = found.iter().filter_map(|v| v["id"].as_str()).collect();
        check!(
            ids.iter().all(|id| found.contains(id.as_str())),
            "projection scan did not confirm imports"
        );
        c.execute(
            "UPDATE ac_outbox SET projected=clock_timestamp() WHERE id=ANY($1)",
            &[&jobs],
        )
        .await?;
    }
}
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let dsn = std::env::var("AC_DATABASE_URL")?;
    let parsed: tokio_postgres::Config = dsn.parse()?;
    check!(
        parsed.get_dbname() == Some("manga_ac_bench"),
        "isolated fixture database required"
    );
    let repo = Repository::connect(DbConfig {
        dsn,
        connections: 16,
        acquisition_timeout: Duration::from_secs(5),
        cache_weight: 64 * 1024 * 1024,
        plaintext_local: true,
    })
    .await?;
    repo.pool.get().await?.batch_execute("CREATE TABLE IF NOT EXISTS ac_accounts(pk text PRIMARY KEY);CREATE TABLE IF NOT EXISTS ac_heads(pk text PRIMARY KEY,event_id text NOT NULL,created_at bigint NOT NULL,event jsonb NOT NULL,refs text[] NOT NULL);CREATE TABLE IF NOT EXISTS ac_objects(hash text PRIMARY KEY,size bigint NOT NULL,current_refs bigint NOT NULL DEFAULT 0 CHECK(current_refs>=0),visible_until timestamptz);CREATE INDEX IF NOT EXISTS ac_objects_live ON ac_objects(hash) WHERE current_refs>0;CREATE TABLE IF NOT EXISTS ac_outbox(id bigserial PRIMARY KEY,event_id text UNIQUE NOT NULL,pk text NOT NULL,event jsonb NOT NULL,refs text[] NOT NULL,accepted timestamptz NOT NULL DEFAULT clock_timestamp(),projected timestamptz);CREATE INDEX IF NOT EXISTS ac_pending ON ac_outbox(id) WHERE projected IS NULL;").await?;
    let root = PathBuf::from(std::env::var("AC_OBJECT_DIR")?);
    std::fs::create_dir_all(&root)?;
    let s = App {
        repo,
        root,
        io: Arc::new(Semaphore::new(4)),
        writes: Arc::new(Semaphore::new(
            std::env::var("AC_WRITE_SLOTS")
                .unwrap_or("16".into())
                .parse()?,
        )),
        cache: Cache::builder()
            .max_capacity(64 * 1024 * 1024)
            .weigher(|_: &String, v: &Arc<Vec<u8>>| v.len() as u32)
            .build(),
        fault: std::env::var("AC_FAULT").unwrap_or_default(),
        max_event: std::env::var("AC_MAX_EVENT")
            .unwrap_or("16777216".into())
            .parse()?,
    };
    let worker = s.clone();
    let binary = std::env::var("AC_STRFRY")?;
    let config = std::env::var("AC_STRFRY_CONFIG")?;
    if s.fault != "after_commit" {
        tokio::spawn(async move {
            if let Err(e) = project(worker, binary, config).await {
                eprintln!("PROJECTOR FAILED: {e:#}");
                std::process::exit(2);
            }
        });
    }
    let router = Router::new()
        .route("/commit", post(commit))
        .route("/head/{pk}", get(head))
        .route("/object/{hash}", get(object))
        .route("/status/{id}", get(status))
        .route("/bundle/{pk}", get(bundle))
        .route("/stats", get(stats))
        .route("/health", get(health))
        .route("/reference-audit", get(reference_audit))
        .route("/measurements", get(measurements))
        .layer(DefaultBodyLimit::max(128 * 1024 * 1024))
        .with_state(s);
    let port = std::env::var("AC_PORT").unwrap_or("58199".into());
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    eprintln!("A/C fixture ready on {port}");
    axum::serve(listener, router).await?;
    Ok(())
}
