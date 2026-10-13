mod support;
use anyhow::{Context, Result};
use manga_node::{
    auth,
    db::{DbConfig, Repository},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    process::{Child, Command},
    time::Duration,
};
use support::*;
const ORIGIN: &str = "http://127.0.0.1:19507";
struct Node(Child);
impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn upload(
    c: &reqwest::Client,
    data: &[u8],
    accepted: &mut BTreeMap<String, usize>,
) -> Result<String> {
    let hash = auth::hash(data);
    let r = c
        .put(format!("{ORIGIN}/upload/{hash}"))
        .header("authorization", blossom(1, ORIGIN, "upload", &hash))
        .body(data.to_vec())
        .send()
        .await?;
    anyhow::ensure!(r.status() == 200, "upload failed: {}", r.text().await?);
    accepted.insert(hash.clone(), data.len());
    Ok(hash)
}
async fn ready(c: &reqwest::Client) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if c.get(ORIGIN)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}
async fn refused_one_byte(c: &reqwest::Client) -> Result<()> {
    let bytes = b"Q";
    let hash = auth::hash(bytes);
    assert_eq!(
        c.put(format!("{ORIGIN}/upload/{hash}"))
            .header("authorization", blossom(1, ORIGIN, "upload", &hash))
            .body(bytes.to_vec())
            .send()
            .await?
            .status(),
        503,
        "quota refusal must remain fail-closed"
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/blobs/{hash}"))
            .header("authorization", blossom(1, ORIGIN, "get", &hash))
            .send()
            .await?
            .status(),
        404,
        "rejected upload cannot become readable"
    );
    Ok(())
}
async fn publish(c: &reqwest::Client, e: &auth::Event) -> Result<reqwest::Response> {
    let body = serde_json::to_vec(e)?;
    let url = format!("{ORIGIN}/publications");
    Ok(c.post(&url)
        .header("authorization", http(1, &url, &body))
        .body(body)
        .send()
        .await?)
}
async fn projected(c: &reqwest::Client, id: &str) -> Result<()> {
    for _ in 0..100 {
        if c.get(format!("{ORIGIN}/events/{id}"))
            .send()
            .await?
            .status()
            == 200
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    anyhow::bail!("publication projection pending")
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL and strfry; run scripts/test-postgres.sh"]
async fn complete_originals_before_publication() -> Result<()> {
    let dsn = std::env::var("MANGA_TEST_DATABASE_URL").context("fixture required")?;
    let cfg: tokio_postgres::Config = dsn.parse()?;
    anyhow::ensure!(cfg.get_dbname() == Some("manga_test"), "fixture required");
    let repo = Repository::connect(DbConfig {
        dsn: dsn.clone(),
        connections: 4,
        acquisition_timeout: Duration::from_secs(2),
        cache_weight: 1048576,
        plaintext_local: true,
    })
    .await?;
    repo.migrate().await?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let dir = tempfile::tempdir()?;
    let key = dir.path().join("key");
    write_key(&key, 11)?;
    let binary = std::env::var_os("STRFRY_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("vendor/strfry/strfry"));
    let backend = std::env::var_os("MANGA_TEST_BACKEND_DIR")
        .map(std::path::PathBuf::from)
        .context("shared fixture backend required")?;
    let config = dir.path().join("config.json");
    std::fs::write(
        &config,
        serde_json::to_vec(
            &json!({"bind":"127.0.0.1:19507","base_url":ORIGIN,"service_key_file":key,"admin_pubkey":public(1),"database_plaintext_local":true,"backend":{"binary":binary,"data_dir":backend,"timeout_seconds":2,"map_bytes":67108864},"storage":{"path":dir.path().join("originals"),"volume":"chapterfixture","physical_bytes":2097152,"staging_bytes":8192,"max_blob_bytes":262144,"formats":["image/png"]}}),
        )?,
    )?;
    let start = || -> Result<Node> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.path().join("node.log"))?;
        Ok(Node(
            Command::new(env!("CARGO_BIN_EXE_manga-node"))
                .args(["serve", config.to_str().unwrap()])
                .env("MANGA_DATABASE_URL", &dsn)
                .env("RUST_LOG", "warn")
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        ))
    };
    let mut node = start()?;
    let c = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    ready(&c)
        .await
        .context(std::fs::read_to_string(dir.path().join("node.log")).unwrap_or_default())?;
    // Expected unique input bytes are tracked from successful HTTP uploads,
    // never inferred from the node's tables or internal accounting counters.
    let mut accepted = BTreeMap::new();
    let image = png();
    let hash = upload(&c, &image, &mut accepted).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{hash}"))
            .send()
            .await?
            .status(),
        404
    );
    let series = cbor(json!({"version":1,"titles":{"en":"Synthetic fixture series"}}));
    let series_hash = upload(&c, &series, &mut accepted).await?;
    let series_event = publication(1, 1002, &series);
    assert!(publish(&c, &series_event).await?.status().is_success());
    projected(&c, &series_event.id).await?;
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{series_hash}"))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        series
    );
    let manifest = cbor(
        json!({"version":1,"series":series_event.id,"language":"en-US","chapter":"1.5","pages":[page(&image),page(&image)]}),
    );
    let mh = upload(&c, &manifest, &mut accepted).await?;
    let event = publication(1, 1001, &manifest);
    let r = publish(&c, &event).await?;
    assert!(r.status().is_success(), "{}", r.text().await?);
    projected(&c, &event.id).await?;
    let r = c.get(format!("{ORIGIN}/originals/{hash}")).send().await?;
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "image/png");
    assert_eq!(r.bytes().await?.as_ref(), image);
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{mh}"))
            .send()
            .await?
            .bytes()
            .await?
            .as_ref(),
        manifest
    );
    let catalogue: Value = c
        .get(format!("{ORIGIN}/catalogue/{}", event.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(catalogue["event"], serde_json::to_value(&event)?);
    assert_eq!(
        catalogue["metadata"]["pages"],
        json!([page(&image), page(&image)])
    );
    // Publishing transfers all staged credit once. A full staging-sized object
    // fits, but neither a duplicate upload nor repeated publication creates room.
    let filler = vec![47u8; 8192];
    let filler_hash = upload(&c, &filler, &mut accepted).await?;
    refused_one_byte(&c).await?;
    upload(&c, &filler, &mut accepted).await?;
    assert!(publish(&c, &event).await?.status().is_success());
    refused_one_byte(&c).await?;
    for _ in 0..2 {
        c.delete(format!("{ORIGIN}/blobs/{filler_hash}"))
            .header("authorization", blossom(1, ORIGIN, "delete", &filler_hash))
            .send()
            .await?
            .error_for_status()?;
        accepted.remove(&filler_hash);
        assert_eq!(
            c.get(format!("{ORIGIN}/blobs/{filler_hash}"))
                .header("authorization", blossom(1, ORIGIN, "get", &filler_hash))
                .send()
                .await?
                .status(),
            404
        );
        // A fresh same-byte generation can consume exactly the released quota.
        upload(&c, &filler, &mut accepted).await?;
        refused_one_byte(&c).await?;
    }
    c.delete(format!("{ORIGIN}/blobs/{filler_hash}"))
        .header("authorization", blossom(1, ORIGIN, "delete", &filler_hash))
        .send()
        .await?
        .error_for_status()?;
    accepted.remove(&filler_hash);
    let duplicate: auth::Event = c
        .get(format!("{ORIGIN}/events/{}", event.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        serde_json::to_value(duplicate)?,
        serde_json::to_value(&event)?
    );
    // Public account-owned comments, nested replies and NIP25 reactions.
    let social_url = format!("{ORIGIN}/social");
    let tags = vec![
        vec!["E".into(), event.id.clone()],
        vec!["K".into(), "1001".into()],
        vec!["P".into(), public(1)],
        vec!["e".into(), event.id.clone()],
        vec!["k".into(), "1001".into()],
        vec!["p".into(), public(1)],
    ];
    let comment = sign_at(
        2,
        1111,
        tags.clone(),
        "<script>not executable markup</script>\nPlain text & Unicode: 漫画".into(),
        auth::now(),
    );
    assert_eq!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&comment)?)
            .await?
            .status(),
        403
    );
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{ORIGIN}/admin/accounts/{}", public(2)),
            &json!({"allowed":true})
        )
        .await?
        .status(),
        200
    );
    let response = post_json(&c, 2, &social_url, &serde_json::to_value(&comment)?).await?;
    assert!(response.status().is_success(), "{}", response.text().await?);
    projected(&c, &comment.id).await?;
    let duplicate: Value = post_json(&c, 2, &social_url, &serde_json::to_value(&comment)?)
        .await?
        .json()
        .await?;
    assert_eq!(duplicate["inserted"], false);
    let mut reply_tags = tags.clone();
    reply_tags[3][1] = comment.id.clone();
    reply_tags[4][1] = "1111".into();
    reply_tags[5][1] = public(2);
    let reply = sign_at(2, 1111, reply_tags, "Reply".into(), auth::now());
    assert!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&reply)?)
            .await?
            .status()
            .is_success()
    );
    projected(&c, &reply.id).await?;
    let reaction = sign_at(
        2,
        7,
        vec![
            vec!["e".into(), event.id.clone()],
            vec!["p".into(), public(1)],
        ],
        "+".into(),
        auth::now(),
    );
    assert!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&reaction)?)
            .await?
            .status()
            .is_success()
    );
    projected(&c, &reaction.id).await?;
    let listed: Value = c
        .get(format!("{ORIGIN}/social?target={}&limit=10", event.id))
        .send()
        .await?
        .json()
        .await?;
    let events = listed["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert!(
        events.contains(&serde_json::to_value(&comment)?),
        "comment plaintext and signed bytes must be preserved"
    );
    assert!(events.contains(&serde_json::to_value(&reply)?));
    assert!(events.contains(&serde_json::to_value(&reaction)?));

    // These successful reactions are separate from the later deletion-before-
    // arrival case. Exercise the advertised social ordering and inclusive bounds
    // over signed bytes, including empty and non-vote plaintext content.
    let stamp = auth::now() - 100;
    let mut reactions = vec![];
    for (content, created) in [
        ("-", stamp),
        ("", stamp),
        ("<img src=x onerror=alert(1)> & 漫画", stamp + 1),
        ("⭐", stamp - 1),
    ] {
        let item = sign_at(
            2,
            7,
            vec![vec!["e".into(), event.id.clone()]],
            content.into(),
            created,
        );
        post_json(&c, 2, &social_url, &serde_json::to_value(&item)?)
            .await?
            .error_for_status()?;
        projected(&c, &item.id).await?;
        reactions.push(item);
    }
    let repeated: Value = post_json(&c, 2, &social_url, &serde_json::to_value(&reactions[0])?)
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(repeated["inserted"], false);
    let mut ties = vec![
        serde_json::to_value(&reactions[0])?,
        serde_json::to_value(&reactions[1])?,
    ];
    ties.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let newest = serde_json::to_value(&reactions[2])?;
    let oldest = serde_json::to_value(&reactions[3])?;
    let ordered = vec![newest.clone(), ties[0].clone(), ties[1].clone(), oldest];
    for (offset, limit, since, until, expected) in [
        (0, 10, stamp - 1, stamp + 1, ordered.clone()),
        (1, 2, stamp - 1, stamp + 1, ordered[1..3].to_vec()),
        (0, 10, stamp, stamp, ties),
        (0, 10, stamp + 1, stamp + 1, vec![newest]),
        (4, 10, stamp - 1, stamp + 1, vec![]),
    ] {
        let page: Value = c
            .get(&social_url)
            .query(&[
                ("target", event.id.clone()),
                ("offset", offset.to_string()),
                ("limit", limit.to_string()),
                ("since", since.to_string()),
                ("until", until.to_string()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        assert_eq!(page["order"], "created_at_desc_id_asc");
        assert_eq!(page["public"], true);
        assert_eq!(
            page["events"],
            json!(expected),
            "signed social page offset={offset}, limit={limit}, since={since}, until={until}"
        );
    }
    for invalid in [
        "limit=0",
        "limit=101",
        "offset=-1",
        "offset=10001",
        "since=-1",
        "since=2&until=1",
    ] {
        assert_eq!(
            c.get(format!("{social_url}?target={}&{invalid}", event.id))
                .send()
                .await?
                .status(),
            400
        );
    }
    let mut wrong_tags = tags;
    wrong_tags[2][1] = public(3);
    let wrong = sign_at(2, 1111, wrong_tags, "Wrong root author".into(), auth::now());
    assert_eq!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&wrong)?)
            .await?
            .status(),
        400
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/social?target={}&limit=1001", event.id))
            .send()
            .await?
            .status(),
        400
    );
    let deletion = sign_at(
        2,
        5,
        vec![
            vec!["e".into(), comment.id.clone()],
            vec!["e".into(), event.id.clone()],
            vec!["k".into(), "1111".into()],
        ],
        "Remove my comment".into(),
        auth::now(),
    );
    let deletion_url = format!("{ORIGIN}/account-deletions");
    let result: Value = post_json(&c, 2, &deletion_url, &serde_json::to_value(&deletion)?)
        .await?
        .json()
        .await?;
    assert_eq!(result["ordinary_withdrawals"], 1);
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", comment.id))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", event.id))
            .send()
            .await?
            .status(),
        200,
        "foreign/protected chapter must remain"
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", reply.id))
            .send()
            .await?
            .status(),
        200,
        "deletion does not cascade through replies"
    );
    assert_eq!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&comment)?)
            .await?
            .status(),
        400
    );
    let result: Value = post_json(&c, 2, &deletion_url, &serde_json::to_value(&deletion)?)
        .await?
        .json()
        .await?;
    assert_eq!(result["ordinary_withdrawals"], 0);
    assert_eq!(
        c.get(format!("{deletion_url}/{}", deletion.id))
            .send()
            .await?
            .status(),
        200
    );
    let unseen = sign_at(
        2,
        7,
        vec![vec!["e".into(), event.id.clone()]],
        "-".into(),
        auth::now(),
    );
    let first_delete = sign_at(
        2,
        5,
        vec![vec!["e".into(), unseen.id.clone()]],
        "Before delivery".into(),
        auth::now(),
    );
    assert_eq!(
        post_json(&c, 2, &deletion_url, &serde_json::to_value(first_delete)?)
            .await?
            .status(),
        202
    );
    assert_eq!(
        post_json(&c, 2, &social_url, &serde_json::to_value(unseen)?)
            .await?
            .status(),
        400
    );
    assert_eq!(
        post_json(
            &c,
            1,
            &format!("{ORIGIN}/admin/accounts/{}", public(2)),
            &json!({"allowed":false})
        )
        .await?
        .status(),
        200
    );
    assert_eq!(
        post_json(&c, 2, &social_url, &serde_json::to_value(&comment)?)
            .await?
            .status(),
        403
    );
    for defect in 0..4 {
        let mut p = page(&image);
        match defect {
            0 => p["width"] = json!(3),
            1 => p["sha256"] = json!("a1".repeat(32)),
            2 => p["mime"] = json!("image/jpeg"),
            _ => p["size"] = json!(image.len() + 1),
        };
        let pages = if defect == 3 {
            json!([page(&image), p])
        } else {
            json!([p])
        };
        let bad = cbor(json!({"version":1,"series":series_event.id,"language":"en","pages":pages}));
        let h = upload(&c, &bad, &mut accepted).await?;
        let e = publication(1, 1001, &bad);
        assert_eq!(publish(&c, &e).await?.status(), 400);
        assert_eq!(
            c.get(format!("{ORIGIN}/events/{}", e.id))
                .send()
                .await?
                .status(),
            404
        );
        assert_eq!(
            c.get(format!("{ORIGIN}/originals/{h}"))
                .send()
                .await?
                .status(),
            404
        );
    }
    let corrupt = &image[..image.len() - 16];
    let h = upload(&c, corrupt, &mut accepted).await?;
    let bad =
        cbor(json!({"version":1,"series":series_event.id,"language":"en","pages":[page(corrupt)]}));
    upload(&c, &bad, &mut accepted).await?;
    assert_eq!(
        publish(&c, &publication(1, 1001, &bad)).await?.status(),
        400
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{h}"))
            .send()
            .await?
            .status(),
        404
    );
    let mut query = url::Url::parse(&format!("{ORIGIN}/query"))?;
    query
        .query_pairs_mut()
        .append_pair("filter", &format!("series={:?}", series_event.id))
        .append_pair("order", "desc");
    let page: Value = c
        .get(query.clone())
        .header("authorization", http_get(1, query.as_str()))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(page["events"][0]["id"], event.id);
    assert_eq!(page["events"].as_array().unwrap().len(), 1);
    let caps: Value = c.get(ORIGIN).send().await?.json().await?;
    assert_eq!(caps["features"]["publication"], true);

    let shared = cbor(
        json!({"version":1,"series":series_event.id,"language":"en","chapter":"2","pages":[support::page(&image)]}),
    );
    upload(&c, &shared, &mut accepted).await?;
    let shared_event = publication(1, 1001, &shared);
    assert!(publish(&c, &shared_event).await?.status().is_success());
    projected(&c, &shared_event.id).await?;
    // Fill the configured physical volume from the known accepted input bytes.
    // This independently catches duplicate charging; after corruption the same
    // one-byte refusal catches premature refund without inspecting a ledger.
    drop(node);
    let mut expanded: Value = serde_json::from_slice(&std::fs::read(&config)?)?;
    expanded["storage"]["staging_bytes"] = json!(2097152);
    std::fs::write(&config, serde_json::to_vec(&expanded)?)?;
    node = start()?;
    ready(&c).await?;
    let mut remaining = 2097152usize
        .checked_sub(accepted.values().sum())
        .context("fixture inputs exceed physical capacity")?;
    let mut salt = 100u8;
    while remaining > 0 {
        let bytes = vec![salt; remaining.min(262144)];
        upload(&c, &bytes, &mut accepted).await?;
        remaining -= bytes.len();
        salt += 1;
    }
    refused_one_byte(&c).await?;
    upload(&c, &image, &mut accepted).await?;
    assert!(publish(&c, &event).await?.status().is_success());
    refused_one_byte(&c).await?;
    let warm: auth::Event = c
        .get(format!("{ORIGIN}/events/{}", event.id))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(serde_json::to_value(warm)?, serde_json::to_value(&event)?);
    let hash_id: [u8; 32] = hex::decode(&hash)?.try_into().unwrap();
    let intent = repo.blob_intent(hash_id).await?.unwrap();
    let path = dir
        .path()
        .join("originals/objects")
        .join(&hash[..2])
        .join(&hash[2..4])
        .join(format!("{}.{}", hash, hex::encode(intent.key.generation)));
    std::fs::write(path, b"synthetic damaged stored original")?;
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{hash}"))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{hash}"))
            .send()
            .await?
            .status(),
        404,
        "repeated corruption observation stays unavailable"
    );
    for e in [&event, &shared_event] {
        assert_eq!(
            c.get(format!("{ORIGIN}/events/{}", e.id))
                .send()
                .await?
                .status(),
            404
        );
    }
    let response = c
        .get(query.clone())
        .header("authorization", http_get(1, query.as_str()))
        .send()
        .await?;
    assert_eq!(response.status(), 200, "{}", response.text().await?);
    let hidden: Value = response.json().await?;
    assert_eq!(hidden["events"].as_array().unwrap().len(), 0);
    assert!(
        repo.inventory_rows_scoped("chapters", "", "")
            .await?
            .is_empty()
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/originals/{mh}"))
            .send()
            .await?
            .status(),
        404
    );
    assert_eq!(
        c.get(format!("{ORIGIN}/events/{}", series_event.id))
            .send()
            .await?
            .status(),
        200
    );
    refused_one_byte(&c).await?;
    drop(node);
    std::fs::write(
        root.join("results/postgres-http-publication.json"),
        serde_json::to_vec_pretty(
            &json!({"passed":true,"checks":["real HTTP series/chapter publication with real strfry","staged originals private until complete publication projects","exact original and deterministic manifest bytes round trip","repeated original validated/referenced once while page count/order preserved","full staging capacity is reused after retention and deletion without duplicate-upload/publication over-credit","full physical quota admits duplicate originals and identities but no new byte","bad dimensions, absent original, disabled format, conflicting descriptors and truncated image cannot become hosted","detected shared corruption gates warm cached events, discovery and inventories without refunding physical storage","successful upvote/downvote/empty/custom reaction and comment plaintext preserve exact signed events","social timestamp-descending and ID-ascending ties, inclusive time bounds, offset/limit and invalid bounds are verified over HTTP"],"limits":["local synthetic PNG fixture","no nginx/static projection yet","moderation and peer replication still later tickets"]}),
        )?,
    )?;
    Ok(())
}
