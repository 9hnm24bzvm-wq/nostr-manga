//! Optional, bounded native-only data for real UI pagination acceptance.
//! Invoked only by the private disposable fixture provisioner, never an HTTP API.
use super::{Request, bounded_json, loopback_origin, write_json};
use anyhow::{Context, Result, ensure};
use manga_node::db::Repository;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Write},
    path::Path,
};
use tokio_postgres::Transaction;
use weebdex_platform::{
    domains::{
        auth::SessionPrincipal,
        catalog::{
            CatalogPage, CatalogRepository, ChapterPublication,
            repository::{audit, create_thread},
        },
    },
    error::ApiResult,
};

const MARKER: &str = "ui-regressions-v1";
const COMPACT: &str = "UiFeedM001";
const TERMINAL: &str = "UiEdgeM001";
const ENTITY: &str = "UiEntM0001";
const ENTITY_USER: &str = "UiEntUser1";
const ENTITY_GROUP: &str = "UiEntGrp01";
const ENTITY_AUTHOR: &str = "UiEntAut01";
const OPERATOR: &str = "SeedStaff1";
const BILINGUAL: &str = "UiBiM00001";
const READERS: &str = "UiReadM001";
const SEARCH_COUNT: i64 = 10_000;

fn service<T>(result: ApiResult<T>) -> Result<T> {
    result.map_err(|error| {
        anyhow::anyhow!("fixture catalogue service rejected action: {}", error.code)
    })
}

#[derive(Clone)]
struct Original {
    object: String,
    dimensions: [i32; 2],
}

async fn actor(tx: &Transaction<'_>, id: &str) -> Result<SessionPrincipal> {
    let row = tx.query_opt("SELECT username,verified,roles,permissions FROM platform.users WHERE id=$1 AND NOT disabled", &[&id]).await?.context("active fixture actor required")?;
    let principal = SessionPrincipal {
        user_id: id.into(),
        username: row.get(0),
        email: String::new(),
        verified: row.get(1),
        roles: serde_json::from_value(row.get::<_, Value>(2))?,
        permissions: serde_json::from_value(row.get::<_, Value>(3))?,
        session_id: "disposable-fixture-operator".into(),
        is_api_client: false,
        authenticated_age_seconds: 0,
        recovery_session: false,
    };
    service(principal.require_verified())?;
    Ok(principal)
}

async fn original(repo: &Repository, root: &Path) -> Result<Original> {
    let client = repo.pool.get().await?;
    let row = client.query_one("SELECT o.id,o.storage_key,o.digest,o.bytes,o.width,o.height FROM platform.chapter_pages p JOIN platform.media_objects o ON o.id=p.object_id WHERE p.chapter_id='SeedChapt2' AND p.position=0 AND o.mime='image/png'", &[]).await?;
    let key: String = row.get(1);
    ensure!(
        key.len() == 64
            && key
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "fixture storage key required"
    );
    let path = root.join("uploads/objects").join(&key[..2]).join(&key);
    let parent = path.parent().context("fixture object shard")?;
    ensure!(
        parent.canonicalize()? == parent,
        "fixture object directory cannot be a link"
    );
    let meta = std::fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file()
            && !meta.file_type().is_symlink()
            && meta.len() <= 32 * 1024 * 1024
            && i64::try_from(meta.len())? == row.get::<_, i64>(3),
        "bounded real fixture original required"
    );
    let bytes = std::fs::read(&path)?;
    ensure!(
        hex::encode(Sha256::digest(&bytes)) == row.get::<_, String>(2),
        "fixture original digest changed"
    );
    ensure!(
        image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()?
            == (600, 900),
        "bounded portrait header required"
    );
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?;
    let dimensions = [row.get(4), row.get(5)];
    ensure!(
        dimensions == [decoded.width() as i32, decoded.height() as i32] && dimensions == [600, 900],
        "actual portrait fixture dimensions required"
    );
    Ok(Original {
        object: row.get(0),
        dimensions,
    })
}

async fn title(
    tx: &Transaction<'_>,
    id: &str,
    name: &str,
    language: &str,
    rating: &str,
    author: Option<&str>,
) -> Result<()> {
    if let Some(row) = tx.query_opt("SELECT metadata->>'fixture_scenario',title,language,content_rating FROM platform.catalog_manga WHERE id=$1", &[&id]).await? {
        ensure!(row.get::<_,Option<String>>(0).as_deref()==Some(MARKER) && row.get::<_,String>(1)==name && row.get::<_,String>(2)==language && row.get::<_,String>(3)==rating, "fixture title identity conflict");
        return Ok(());
    }
    let metadata = json!({"fixture_scenario":MARKER,"description":"Generated native-only UI acceptance data; no signed protocol claim.","links":{}});
    tx.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,metadata,created_at,updated_at,published_at) VALUES($1,'SeedStaff1','FixtureSeeder',$2,$3,'ongoing',$4,'published',$5,TIMESTAMPTZ '2026-01-01 00:00:00Z',TIMESTAMPTZ '2026-01-01 00:00:00Z',TIMESTAMPTZ '2026-01-01 00:00:00Z')", &[&id,&name,&language,&rating,&metadata]).await?;
    tx.execute(
        "INSERT INTO platform.catalog_manga_counters(manga_id) VALUES($1)",
        &[&id],
    )
    .await?;
    if let Some(author) = author {
        tx.execute("INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role) VALUES($1,$2,'author')", &[&id,&author]).await?;
    }
    service(create_thread(tx, "manga", id, OPERATOR, "FixtureSeeder", name).await)?;
    service(
        audit(
            tx,
            OPERATOR,
            "manga",
            id,
            "fixture_native_seed",
            None,
            Some(1),
        )
        .await,
    )?;
    Ok(())
}

async fn chapters(
    repo: &Repository,
    manga: &str,
    prefix: &str,
    total: usize,
    language: &str,
    uploader: &str,
    groups: &[String],
    origin: &str,
    originals: &[Original],
) -> Result<()> {
    ensure!(
        total <= 505 && !originals.is_empty() && originals.len() <= 8 && prefix.len() == 4,
        "fixture chapter batch bounds"
    );
    for start in (1..=total).step_by(25) {
        let mut client = repo.pool.get().await?;
        let tx = client.transaction().await?;
        let principal = actor(&tx, uploader).await?;
        service(CatalogRepository::ensure_upload_target(&tx, manga, &principal, groups).await)?;
        for number in start..=(start + 24).min(total) {
            let id = format!("{prefix}{number:06}");
            let pages: Vec<_> = originals
                .iter()
                .enumerate()
                .map(|(position, original)| CatalogPage {
                    name: format!("UiRd{position:06}.png"),
                    dimensions: original.dimensions,
                })
                .collect();
            if let Some(row) = tx.query_opt("SELECT manga_id,uploader_id,language,metadata->>'fixture_scenario',deleted_at IS NULL FROM platform.catalog_chapters WHERE id=$1", &[&id]).await? {
                ensure!(row.get::<_,String>(0)==manga && row.get::<_,String>(1)==uploader && row.get::<_,String>(2)==language && row.get::<_,Option<String>>(3).as_deref()==Some(MARKER) && row.get::<_,bool>(4), "fixture chapter retry identity conflict");
                continue;
            }
            service(
                CatalogRepository::publish_chapter(
                    &tx,
                    &ChapterPublication {
                        id: id.clone(),
                        manga_id: manga.into(),
                        uploader_id: uploader.into(),
                        uploader_name: principal.username.clone(),
                        title: Some(format!("UI acceptance chapter {number:04}")),
                        volume: Some("1".into()),
                        chapter: Some(number.to_string()),
                        language: language.into(),
                        node: origin.into(),
                        pages: pages.clone(),
                        optimized_pages: vec![],
                        groups: groups.to_vec(),
                        source_id: None,
                        protocol_event_id: None,
                    },
                )
                .await,
            )?;
            for (position, original) in originals.iter().enumerate() {
                tx.execute("INSERT INTO platform.chapter_pages(chapter_id,name,object_id,position) VALUES($1,$2,$3,$4)", &[&id,&pages[position].name,&original.object,&(position as i32)]).await?;
            }
            let ordinal = (total - number) as i64;
            tx.execute("UPDATE platform.catalog_chapters SET metadata=metadata || jsonb_build_object('fixture_scenario',$3::text),created_at=TIMESTAMPTZ '2026-01-01 00:00:00Z'+$2::bigint*INTERVAL '1 second',updated_at=TIMESTAMPTZ '2026-01-01 00:00:00Z'+$2::bigint*INTERVAL '1 second',published_at=TIMESTAMPTZ '2026-01-01 00:00:00Z'+$2::bigint*INTERVAL '1 second' WHERE id=$1", &[&id,&ordinal,&MARKER]).await?;
        }
        tx.commit().await?;
    }
    Ok(())
}

pub(super) async fn seed(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    ensure!(
        request.scenario.is_none() || request.scenario.as_deref() == Some("pagination"),
        "only named pagination scenario may be seeded"
    );
    if root.join("ui-regressions.json").exists() {
        return status(repo, root).await;
    }
    let origin = loopback_origin(request.ui_origin.as_deref())?;
    let portrait = original(repo, root).await?;
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    let operator = actor(&tx, OPERATOR).await?;
    ensure!(operator.is_staff(), "persisted fixture staff required");
    service(operator.require_permission("chapter.create"))?;
    tx.execute("INSERT INTO platform.users(id,username,email,password_hash,verified,roles,permissions,profile) VALUES('UiEntUser1','UIAcceptanceUploader','ui-acceptance-uploader@example.invalid','not-a-login-password-hash',TRUE,'[]','[\"chapter.create\",\"self.chapter.remove\"]',jsonb_build_object('fixture_scenario',$1::text)) ON CONFLICT(id) DO NOTHING", &[&MARKER]).await?;
    let seeded_user = tx
        .query_one(
            "SELECT username,profile->>'fixture_scenario' FROM platform.users WHERE id=$1",
            &[&ENTITY_USER],
        )
        .await?;
    ensure!(
        seeded_user.get::<_, String>(0) == "UIAcceptanceUploader"
            && seeded_user.get::<_, Option<String>>(1).as_deref() == Some(MARKER),
        "fixture uploader identity conflict"
    );
    for (id, name, table) in [
        ("UiFeedGrp1", "UI compact feed group", "catalog_groups"),
        (ENTITY_GROUP, "UI entity pagination group", "catalog_groups"),
        ("UiFeedAut1", "UI compact feed author", "catalog_authors"),
        (
            ENTITY_AUTHOR,
            "UI entity pagination author",
            "catalog_authors",
        ),
    ] {
        tx.execute(&format!("INSERT INTO platform.{table}(id,owner_id,owner_name,name,metadata) VALUES($1,'SeedStaff1','FixtureSeeder',$2,jsonb_build_object('fixture_scenario',$3::text)) ON CONFLICT(id) DO NOTHING"), &[&id,&name,&MARKER]).await?;
        let row = tx.query_one(&format!("SELECT metadata->>'fixture_scenario' FROM platform.{table} WHERE id=$1 AND deleted_at IS NULL"), &[&id]).await?;
        ensure!(
            row.get::<_, Option<String>>(0).as_deref() == Some(MARKER),
            "fixture entity identity conflict"
        );
        if table=="catalog_groups" && !tx.query_one("SELECT EXISTS(SELECT 1 FROM platform.catalog_threads WHERE entity_type='group' AND entity_id=$1)", &[&id]).await?.get::<_,bool>(0) {
            service(create_thread(&tx,"group",id,OPERATOR,"FixtureSeeder",name).await)?;
        }
    }
    title(
        &tx,
        COMPACT,
        "UI compact feed fixture",
        "ja",
        "safe",
        Some("UiFeedAut1"),
    )
    .await?;
    title(
        &tx,
        TERMINAL,
        "UI terminal page fixture",
        "ja",
        "safe",
        None,
    )
    .await?;
    title(
        &tx,
        ENTITY,
        "UI entity upload fixture",
        "ja",
        "safe",
        Some(ENTITY_AUTHOR),
    )
    .await?;
    for (id, name, lang, rating) in [
        ("UiPfSafe01", "UI preference safe fixture", "ja", "safe"),
        (
            "UiPfErot01",
            "UI preference erotic fixture",
            "ja",
            "erotica",
        ),
        (
            "UiPfPorn01",
            "UI preference adult fixture",
            "ja",
            "pornographic",
        ),
        ("UiPfLang01", "UI preference language fixture", "es", "safe"),
    ] {
        title(&tx, id, name, lang, rating, None).await?;
    }
    // Newly created native fixture titles get actual seed-time recency. Most
    // Read acceptance must produce genuine browser analytics POSTs, not counters.
    tx.execute("UPDATE platform.catalog_manga SET created_at=clock_timestamp(),updated_at=clock_timestamp(),published_at=clock_timestamp() WHERE id=ANY($1) AND metadata->>'fixture_scenario'=$2", &[&vec!["UiPfSafe01","UiPfErot01","UiPfPorn01","UiPfLang01"],&MARKER]).await?;
    // Reuse the real base-fixture genre for visible blacklist invalidation.
    tx.execute("INSERT INTO platform.catalog_manga_tags(manga_id,tag_id) VALUES('UiPfLang01','SeedGenre1') ON CONFLICT(manga_id,tag_id) DO NOTHING",&[]).await?;
    tx.commit().await?;
    drop(client);
    chapters(
        repo,
        COMPACT,
        "UiCp",
        505,
        "es",
        OPERATOR,
        &["UiFeedGrp1".into()],
        &origin,
        &[portrait.clone()],
    )
    .await?;
    chapters(
        repo,
        TERMINAL,
        "UiTm",
        101,
        "fr",
        OPERATOR,
        &[],
        &origin,
        &[portrait.clone()],
    )
    .await?;
    chapters(
        repo,
        ENTITY,
        "UiEn",
        101,
        "ja",
        ENTITY_USER,
        &[ENTITY_GROUP.into()],
        &origin,
        &[portrait],
    )
    .await?;
    // Metadata-only count-cap input is separate from translated chapter feeds.
    // No invented popularity, activity, media or protocol source is attached.
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    tx.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state,metadata,created_at,updated_at,published_at) SELECT 'UiSr'||lpad(n::text,6,'0'),'SeedStaff1','FixtureSeeder','UI16CapFixture '||lpad(n::text,6,'0'),'ja','ongoing','safe','published',jsonb_build_object('fixture_scenario',$1::text,'description','Generated metadata-only native UI count-cap fixture.'),TIMESTAMPTZ '2026-01-01 00:00:00Z'+n*INTERVAL '1 second',TIMESTAMPTZ '2026-01-01 00:00:00Z'+n*INTERVAL '1 second',TIMESTAMPTZ '2026-01-01 00:00:00Z'+n*INTERVAL '1 second' FROM generate_series(1,$2::integer) n ON CONFLICT(id) DO NOTHING", &[&MARKER,&(SEARCH_COUNT as i32)]).await?;
    let search_count: i64 = tx.query_one("SELECT count(*) FROM platform.catalog_manga WHERE id ~ '^UiSr[0-9]{6}$' AND metadata->>'fixture_scenario'=$1 AND state='published' AND deleted_at IS NULL AND title LIKE 'UI16CapFixture %'", &[&MARKER]).await?.get(0);
    ensure!(
        search_count == SEARCH_COUNT,
        "exact metadata search fixture required"
    );
    let search_bytes: i64=tx.query_one("SELECT COALESCE(sum(octet_length(title)+pg_column_size(metadata)),0)::bigint FROM platform.catalog_manga WHERE id ~ '^UiSr[0-9]{6}$' AND metadata->>'fixture_scenario'=$1", &[&MARKER]).await?.get(0);
    ensure!(
        search_bytes <= 4 * 1024 * 1024,
        "metadata-only search fixture byte budget"
    );
    tx.execute("INSERT INTO platform.catalog_manga_counters(manga_id) SELECT id FROM platform.catalog_manga WHERE id ~ '^UiSr[0-9]{6}$' AND metadata->>'fixture_scenario'=$1 ON CONFLICT(manga_id) DO NOTHING", &[&MARKER]).await?;
    tx.commit().await?;
    let descriptor = json!({"fixture_only":true,"native_only":true,"protocol_signed_publications":false,"scenario":MARKER,
        "compact":{"manga_id":COMPACT,"group_id":"UiFeedGrp1","author_id":"UiFeedAut1","total":505,"language":"es"},
        "terminal":{"manga_id":TERMINAL,"total":101,"withdraw_chapter_id":"UiTm000101","language":"fr"},
        "entity":{"manga_id":ENTITY,"group_id":ENTITY_GROUP,"author_id":ENTITY_AUTHOR,"user_id":ENTITY_USER,"total":101,"shrink_chapter_id":"UiEn000101","language":"ja"},
        "search":{"query":"UI16CapFixture","total":SEARCH_COUNT,"metadata_only":true,"title_and_json_bytes":search_bytes,"title_and_json_byte_budget":4194304},
        "preference":{"safe_id":"UiPfSafe01","erotic_id":"UiPfErot01","pornographic_id":"UiPfPorn01","language_title_id":"UiPfLang01","language":"es","tag_id":"SeedGenre1","tagged_title_id":"UiPfLang01","most_read_requires_genuine_analytics_posts":true},
        "reader_activation":{"action":"ui-regressions-readers","must_follow_compact_es_checks":true},
        "withdrawal":{"action":"ui-regressions-withdraw","scenarios":["terminal","entity"],"production_catalogue_deletion_service":true},
        "extra_original_bytes":0,"bulk_chapter_originals_reused":true});
    write_json(&root.join("ui-regressions.json"), &descriptor)?;
    status(repo, root).await
}

async fn landscape(repo: &Repository, root: &Path) -> Result<Original> {
    let image = image::RgbImage::from_fn(900, 600, |x, y| {
        image::Rgb([((x / 8) % 240) as u8, ((y / 6) % 240) as u8, 80])
    });
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image).write_to(&mut encoded, image::ImageFormat::Png)?;
    let bytes = encoded.into_inner();
    ensure!(
        bytes.len() <= 1024 * 1024,
        "synthetic landscape storage bound"
    );
    let digest = hex::encode(Sha256::digest(&bytes));
    let key = hex::encode(Sha256::digest(format!("ui-regression:{digest}").as_bytes()));
    let objects = root.join("uploads/objects");
    ensure!(
        objects.canonicalize()? == objects
            && !std::fs::symlink_metadata(&objects)?
                .file_type()
                .is_symlink(),
        "fixture objects root cannot be a link"
    );
    let directory = objects.join(&key[..2]);
    std::fs::create_dir_all(&directory)?;
    ensure!(
        directory.canonicalize()? == directory,
        "fixture object shard cannot be a link"
    );
    let path = directory.join(&key);
    if path.exists() {
        let metadata = std::fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() <= 1024 * 1024,
            "regular bounded landscape object required"
        );
        ensure!(
            std::fs::read(&path)? == bytes,
            "fixture landscape identity changed"
        );
    } else {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::File::open(&directory)?.sync_all()?;
        std::fs::File::open(&objects)?.sync_all()?;
    }
    repo.pool.get().await?.execute("INSERT INTO platform.media_objects(id,digest,storage_key,mime,extension,bytes,width,height) VALUES($1,$1,$2,'image/png','png',$3,900,600) ON CONFLICT(id) DO NOTHING", &[&digest,&key,&(bytes.len() as i64)]).await?;
    Ok(Original {
        object: digest,
        dimensions: [900, 600],
    })
}

pub(super) async fn readers(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    ensure!(
        request.scenario.is_none() || request.scenario.as_deref() == Some("readers"),
        "only named reader scenario may be seeded"
    );
    let pagination = bounded_json(&root.join("ui-regressions.json"))?;
    ensure!(
        pagination["scenario"] == MARKER,
        "pagination fixture must precede readers"
    );
    if root.join("ui-regression-readers.json").exists() {
        return status(repo, root).await;
    }
    let origin = loopback_origin(request.ui_origin.as_deref())?;
    let portrait = original(repo, root).await?;
    let landscape = landscape(repo, root).await?;
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    title(
        &tx,
        BILINGUAL,
        "UI bilingual reader fixture",
        "ja",
        "safe",
        None,
    )
    .await?;
    title(&tx, READERS, "UI spread reader fixture", "ja", "safe", None).await?;
    tx.commit().await?;
    drop(client);
    chapters(
        repo,
        BILINGUAL,
        "UiBe",
        1,
        "en",
        OPERATOR,
        &[],
        &origin,
        &[portrait.clone()],
    )
    .await?;
    chapters(
        repo,
        BILINGUAL,
        "UiBs",
        1,
        "es",
        OPERATOR,
        &[],
        &origin,
        &[portrait.clone()],
    )
    .await?;
    let four = vec![portrait.clone(); 4];
    let eight = vec![
        portrait.clone(),
        portrait.clone(),
        landscape.clone(),
        portrait.clone(),
        portrait.clone(),
        portrait.clone(),
        landscape,
        portrait,
    ];
    chapters(
        repo,
        READERS,
        "UiR4",
        1,
        "en",
        OPERATOR,
        &[],
        &origin,
        &four,
    )
    .await?;
    chapters(
        repo,
        READERS,
        "UiR8",
        1,
        "en",
        OPERATOR,
        &[],
        &origin,
        &eight,
    )
    .await?;
    let descriptor = json!({"fixture_only":true,"bilingual":{"manga_id":BILINGUAL,"english_chapter_id":"UiBe000001","spanish_chapter_id":"UiBs000001"},
        "readers":[{"chapter_id":"UiR4000001","manga_id":READERS,"pages":four.iter().map(|p|json!({"dimensions":p.dimensions})).collect::<Vec<_>>()},{"chapter_id":"UiR8000001","manga_id":READERS,"pages":eight.iter().map(|p|json!({"dimensions":p.dimensions})).collect::<Vec<_>>()}],
        "real_generated_landscape_original":true,"extra_original_byte_budget":1048576,"compact_es_count_after_activation":506});
    write_json(&root.join("ui-regression-readers.json"), &descriptor)?;
    status(repo, root).await
}

pub(super) async fn withdraw(repo: &Repository, root: &Path, request: &Request) -> Result<Value> {
    let descriptor = bounded_json(&root.join("ui-regressions.json"))?;
    ensure!(
        descriptor["scenario"] == MARKER,
        "recorded UI fixture required"
    );
    let (manga, id, uploader) = match request.scenario.as_deref() {
        Some("terminal") => (TERMINAL, "UiTm000101", OPERATOR),
        Some("entity") => (ENTITY, "UiEn000101", ENTITY_USER),
        _ => anyhow::bail!("only named terminal/entity withdrawal is allowed"),
    };
    let mut client = repo.pool.get().await?;
    let tx = client.transaction().await?;
    let principal = actor(&tx, OPERATOR).await?;
    ensure!(
        principal.is_staff(),
        "current persisted fixture staff required"
    );
    service(principal.require_permission("chapter.remove"))?;
    let row=tx.query_one("SELECT manga_id,uploader_id,metadata->>'fixture_scenario',protocol_event_id,deleted_at IS NOT NULL FROM platform.catalog_chapters WHERE id=$1 FOR UPDATE", &[&id]).await?;
    ensure!(
        row.get::<_, String>(0) == manga
            && row.get::<_, String>(1) == uploader
            && row.get::<_, Option<String>>(2).as_deref() == Some(MARKER)
            && row.get::<_, Option<String>>(3).is_none(),
        "exact native fixture target required"
    );
    let replay: bool = row.get(4);
    if !replay {
        service(CatalogRepository::delete_chapter(&tx, id, &principal).await)?;
    }
    tx.commit().await?;
    let current = status(repo, root).await?;
    Ok(
        json!({"fixture_only":true,"action":"ui-regressions-withdraw","scenario":request.scenario,"chapter_id":id,"instance_removal":"soft-hidden","production_catalogue_deletion_service":true,"idempotent_replay":replay,"ui_regressions":current}),
    )
}

pub(super) async fn status(repo: &Repository, root: &Path) -> Result<Value> {
    let mut descriptor = bounded_json(&root.join("ui-regressions.json"))?;
    ensure!(
        descriptor["fixture_only"] == true && descriptor["scenario"] == MARKER,
        "recorded native UI scenario required"
    );
    let client = repo.pool.get().await?;
    for (key, manga) in [
        ("compact", COMPACT),
        ("terminal", TERMINAL),
        ("entity", ENTITY),
    ] {
        let count: i64 = client.query_one("SELECT count(*) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.manga_id=$1 AND c.deleted_at IS NULL AND NOT c.is_unavailable AND m.deleted_at IS NULL AND m.state='published' AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id) AND platform.protocol_source_is_visible(c.protocol_event_id)", &[&manga]).await?.get(0);
        descriptor[key]["current_total"] = json!(count);
    }
    if root.join("ui-regression-readers.json").exists() {
        let readers = bounded_json(&root.join("ui-regression-readers.json"))?;
        descriptor["bilingual"] = readers["bilingual"].clone();
        descriptor["readers"] = readers["readers"].clone();
        descriptor["reader_activation"]["activated"] = json!(true);
    } else {
        descriptor["reader_activation"]["activated"] = json!(false);
    }
    Ok(descriptor)
}
