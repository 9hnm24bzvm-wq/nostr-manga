//! Query semantics fixture uses explicitly injected synthetic signed metadata.
//! It does not claim that these rows passed original/media admission.
mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use std::{
    process::{Child, Command},
    time::Duration,
};
use support::*;
const ORIGIN: &str = "http://127.0.0.1:19508";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn query(c: &reqwest::Client, args: &[(&str, String)]) -> Result<reqwest::Response> {
    let mut url = url::Url::parse(&format!("{ORIGIN}/query"))?;
    url.query_pairs_mut()
        .extend_pairs(args.iter().map(|(k, v)| (*k, v.as_str())));
    if !args.iter().any(|(k, _)| *k == "order") {
        url.query_pairs_mut().append_pair("order", "desc");
    }
    let raw = url.to_string();
    Ok(c.get(url)
        .header("authorization", http_get(1, &raw))
        .send()
        .await?)
}
async fn inject(
    repo: &Repository,
    n: u64,
    time: u64,
    chapter: Option<&str>,
    content: Option<String>,
) -> Result<auth::Event> {
    let event = sign_at(
        1,
        1001,
        vec![vec!["fixture".into(), n.to_string()]],
        content.unwrap_or_else(|| json!({"manifest":format!("{n:064x}"),"version":1}).to_string()),
        time,
    );
    repo.pool.get().await?.execute("INSERT INTO events(id,publisher,kind,created_at,raw_event,state,series,language,chapter,pages,size) VALUES($1,$2,1001,$3,$4,'hosted',$5,'en-US',$6,2,100)", &[&hex::decode(&event.id)?,&hex::decode(&event.pubkey)?,&(time as i64),&serde_json::to_value(&event)?,&vec![9u8;32],&chapter]).await?;
    Ok(event)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run scripts/test-postgres.sh"]
async fn accepted_query_semantics_over_http() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 8,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19508","base_url":ORIGIN,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true}),
        )?,
    )?;
    let log = std::fs::File::create(dir.path().join("node.log"))?;
    let _node = Node(
        Command::new(env!("CARGO_BIN_EXE_manga-node"))
            .args(["serve", config.to_str().unwrap()])
            .env("MANGA_DATABASE_URL", &dsn)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let mut ready = false;
    for _ in 0..60 {
        if c.get(ORIGIN).send().await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready);
    let mut expected = vec![];
    for n in 0..12 {
        expected.push(
            inject(
                &repo,
                n,
                100 + n / 2,
                if n % 3 == 0 { None } else { Some("Alpha") },
                None,
            )
            .await?,
        );
    }
    expected.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    let mut got = vec![];
    for offset in [0, 4, 8] {
        let response = query(
            &c,
            &[
                ("offset", offset.to_string()),
                ("limit", "4".into()),
                ("order", "desc".into()),
            ],
        )
        .await?;
        assert_eq!(response.status(), 200);
        let page: Value = response.json().await?;
        assert_eq!(page["snapshot"], false);
        assert_eq!(page["defaults_provisional"], false);
        assert!(page.get("total").is_none());
        assert_eq!(page["has_more"], offset < 8);
        for e in page["events"].as_array().unwrap() {
            let e: auth::Event = serde_json::from_value(e.clone())?;
            e.verify().map_err(anyhow::Error::msg)?;
            got.push(e.id);
        }
    }
    assert_eq!(
        got,
        expected.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
    );
    let no_order = format!("{ORIGIN}/query?limit=2");
    assert_eq!(
        c.get(&no_order)
            .header("authorization", http_get(1, &no_order))
            .send()
            .await?
            .status(),
        400
    );
    let counted: Value = query(
        &c,
        &[
            ("limit", "2".into()),
            ("offset", "4".into()),
            ("include_total", "true".into()),
        ],
    )
    .await?
    .json()
    .await?;
    assert_eq!(counted["total"], 12);
    assert_eq!(counted["events"].as_array().unwrap().len(), 2);
    let counted: Value = query(
        &c,
        &[
            ("filter", "missing(chapter)".into()),
            ("include_total", "true".into()),
        ],
    )
    .await?
    .json()
    .await?;
    assert_eq!(counted["total"], 4);
    let ascending: Value = query(&c, &[("order", "asc".into())]).await?.json().await?;
    let mut asc = expected.clone();
    asc.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    assert_eq!(
        ascending["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        asc.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
    );
    for (literal, count) in [(public(1), 12), (public(1).to_ascii_uppercase(), 0)] {
        let p: Value = query(&c, &[("filter", format!("publisher={literal:?}"))])
            .await?
            .json()
            .await?;
        assert_eq!(p["events"].as_array().unwrap().len(), count);
    }
    let page: Value = query(
        &c,
        &[
            ("since", "102".into()),
            ("until", "102".into()),
            ("order", "asc".into()),
            ("filter", "language=\"EN\" AND pages>=2".into()),
        ],
    )
    .await?
    .json()
    .await?;
    assert_eq!(page["events"].as_array().unwrap().len(), 2);
    for (filter, count) in [
        ("chapter=\"Alpha\"", 8),
        ("chapter=\"alpha\"", 0),
        ("NOT chapter=\"Alpha\"", 0),
        ("missing(chapter)", 4),
        ("exists(chapter)", 8),
        ("language=\"en-US\"", 12),
        ("language=\"en-U\"", 0),
    ] {
        let page: Value = query(&c, &[("filter", filter.into())])
            .await?
            .json()
            .await?;
        assert_eq!(page["events"].as_array().unwrap().len(), count, "{filter}");
    }
    for args in [
        vec![("limit", "0".into())],
        vec![("limit", "2".into()), ("limit", "3".into())],
        vec![("count", "exact".into())],
        vec![("since", "10".into()), ("until", "9".into())],
    ] {
        assert_eq!(query(&c, &args).await?.status(), 400);
    }
    let wrong = format!("{ORIGIN}/query?limit=2");
    assert_eq!(
        c.get(format!("{ORIGIN}/query?limit=3"))
            .header("authorization", http_get(1, &wrong))
            .send()
            .await?
            .status(),
        403
    );
    // A newly inserted newest event shifts a live offset page; no snapshot promise.
    let first: Value = query(&c, &[("limit", "4".into())]).await?.json().await?;
    let last = first["events"][3]["id"].clone();
    inject(&repo, 99, 500, Some("new"), None).await?;
    let second: Value = query(&c, &[("limit", "4".into()), ("offset", "4".into())])
        .await?
        .json()
        .await?;
    assert_eq!(second["events"][0]["id"], last);
    // Work-budget failure returns an error rather than an unexplained partial page.
    let mut locked = repo.pool.get().await?;
    let tx = locked.transaction().await?;
    tx.batch_execute("LOCK TABLE events IN ACCESS EXCLUSIVE MODE")
        .await?;
    let response = query(
        &c,
        &[("limit", "2".into()), ("include_total", "true".into())],
    )
    .await?;
    assert_eq!(response.status(), 503);
    let failure: Value = response.json().await?;
    assert!(failure.get("events").is_none());
    assert!(failure.get("total").is_none());
    tx.rollback().await?;
    assert_eq!(query(&c, &[("limit", "2".into())]).await?.status(), 200);
    for n in 200..203 {
        inject(&repo, n, 600, Some("oversized"), Some("x".repeat(800_000))).await?;
    }
    assert_eq!(
        query(&c, &[("filter", "chapter=\"oversized\"".into())])
            .await?
            .status(),
        503
    );
    // SQL only arranges synthetic catalogue inputs; HTTP IDs are the outcome oracle.
    let mut samples = Vec::new();
    for (n, language, chapter) in [
        (400, Some("de-DE"), Some("A")),
        (401, Some("de-DE-1996"), Some("a")),
        (402, Some("de-Deva"), None),
        (403, None, None),
    ] {
        let event = inject(&repo, n, 700, chapter, None).await?;
        repo.pool
            .get()
            .await?
            .execute(
                "UPDATE events SET language=$2 WHERE id=$1",
                &[&hex::decode(&event.id)?, &language],
            )
            .await?;
        samples.push(event.id);
    }
    for (expression, selected) in [
        ("language=\"DE-de\"", vec![0, 1]),
        ("NOT language=\"*\"", vec![]),
        ("NOT chapter=\"A\"", vec![1]),
        ("missing(chapter)", vec![2, 3]),
        ("exists(chapter)", vec![0, 1]),
    ] {
        let response = query(
            &c,
            &[
                ("filter", expression.into()),
                ("since", "700".into()),
                ("until", "700".into()),
            ],
        )
        .await?;
        assert_eq!(response.status(), 200, "{expression}");
        let body: Value = response.json().await?;
        let mut ids: Vec<_> = body["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_str().unwrap().to_owned())
            .collect();
        let mut expected: Vec<_> = selected.into_iter().map(|n| samples[n].clone()).collect();
        ids.sort();
        expected.sort();
        assert_eq!(ids, expected, "{expression}");
    }
    for (field, count) in [
        ("series", 4),
        ("publisher", 4),
        ("language", 3),
        ("pages", 4),
        ("size", 4),
        ("created_at", 4),
        ("chapter", 2),
        ("volume", 0),
        ("title", 0),
    ] {
        let response = query(
            &c,
            &[
                ("filter", format!("exists({field})")),
                ("since", "700".into()),
                ("until", "700".into()),
            ],
        )
        .await?;
        assert_eq!(response.status(), 200, "{field}");
        let body: Value = response.json().await?;
        assert_eq!(body["events"].as_array().unwrap().len(), count, "{field}");
    }
    let literal = "x'); DROP TABLE events; --";
    let injected = inject(&repo, 404, 701, Some(literal), None).await?;
    let response = query(
        &c,
        &[(
            "filter",
            format!("chapter={}", serde_json::to_string(literal)?),
        )],
    )
    .await?;
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await?;
    assert_eq!(body["events"].as_array().unwrap().len(), 1);
    assert_eq!(body["events"][0]["id"], injected.id);
    for (range, count) in [
        ("*", 3),
        ("en", 0),
        ("EN-us", 0),
        ("x", 0),
        ("en-a", 0),
        ("de-CH-1996", 0),
    ] {
        let response = query(
            &c,
            &[
                ("filter", format!("language={range:?}")),
                ("since", "700".into()),
                ("until", "700".into()),
            ],
        )
        .await?;
        assert_eq!(response.status(), 200, "{range}");
        let body: Value = response.json().await?;
        assert_eq!(body["events"].as_array().unwrap().len(), count, "{range}");
    }
    let mut rejected: Vec<String> = [
        "pages=\"2\"",
        "chapter>\"a\"",
        "unknown=1",
        "pages=1 trailing",
        "NOT",
        "(pages=1",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for range in ["", "123", "en--US", "en-*", "*-CH", "é"] {
        rejected.push(format!("language={range:?}"));
    }
    rejected.push("x".repeat(8193));
    rejected.push(format!("{}pages=1{}", "(".repeat(21), ")".repeat(21)));
    for filter in rejected {
        assert_eq!(
            query(&c, &[("filter", filter.clone())]).await?.status(),
            400,
            "{filter}"
        );
    }
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results/postgres-http-discovery.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"scope":"synthetic signed metadata fixture, not media admission or target capacity","checks":["explicit order omission rejected","optional exact totals match filtered snapshot independent of page offset","count budget failure returns neither page nor partial total","live offset traversal and deterministic timestamp ties","inclusive timestamp bounds intersect language/numeric filters","exact text and basic language range matching","three-valued missing comparisons and NOT plus exists/missing","invalid and duplicate query parameters rejected","full URL authentication binding","concurrent insertion exhibits selected live-offset repetition","SQL work timeout and oversized page fail without partial result","HTTP identifier sets preserve NULL truth, injection literals and filter budgets"],"decisions":{"Q93":"D: explicit order required","Q94":"A: optional exact totals"}}),
        )?,
    )?;
    Ok(())
}
