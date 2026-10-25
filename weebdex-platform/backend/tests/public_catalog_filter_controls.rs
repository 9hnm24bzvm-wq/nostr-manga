//! Prospective actual TCP + isolated PostgreSQL acceptance. Not a mocked
//! transport and not evidence until the owned native runner executes it.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use weebdex_platform::{AppState, Config, domains, public_api::{PostgresPublicIdResolver, PublicIdResolver, PublicUuid, ResourceKind}};
// Both controls share the guarded isolated database and alias-count assertions.
static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server { fn drop(&mut self) { self.0.abort(); } }
async fn fixture() -> (AppState, Server, reqwest::Client, String) {
    let dsn=std::env::var("PLATFORM_AUTH_TEST_DATABASE_URL").expect("Use isolated runner");
    let pg:tokio_postgres::Config=dsn.parse().unwrap();
    assert_eq!(pg.get_dbname(),Some("weebdex_platform_test"));
    assert!(pg.get_hosts().iter().all(|host|matches!(host,tokio_postgres::config::Host::Tcp(host)if host=="127.0.0.1")));
    let runtime=std::path::PathBuf::from(std::env::var("PLATFORM_TEST_RUNTIME_DIR").unwrap());
    assert!(runtime.to_str().unwrap().contains("/weebdex-platform/test-runtime/"));
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base=format!("http://{}",listener.local_addr().unwrap());
    let state=AppState::connect(Config{database_plaintext_local:true,database_connections:4,public_origin:base.clone(),uploads_dir:runtime.join("media"),ui_dir:runtime.join("no-ui"),..Config::default()},dsn).await.unwrap();
    state.db.migrate(domains::MIGRATIONS).await.unwrap();state.db.verify_schema(domains::MIGRATIONS).await.unwrap();
    let app=weebdex_platform::router::router(state.clone());
    let server=Server(tokio::spawn(async move{axum::serve(listener,app).await.unwrap()}));
    (state,server,reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(20)).build().unwrap(),base)
}
async fn get(client:&reqwest::Client,base:&str,path:&str,status:u16)->Value {
    let response=client.get(format!("{base}/api/v1{path}")).send().await.unwrap();
    let actual=response.status().as_u16();let body:Value=response.json().await.unwrap();assert_eq!(actual,status,"{path}: {body}");body
}
fn ids(body:&Value)->BTreeSet<String>{body["data"].as_array().unwrap().iter().map(|row|row["id"].as_str().unwrap().to_owned()).collect()}
#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="Requires explicitly isolated PostgreSQL runner"]
async fn actual_year_bounds_count_page_mixed_visibility_and_exact_controls(){
    let _guard=TEST_LOCK.lock().await;
    let(state,_server,client,base)=fixture().await;let db=state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.users(id,username,email,password_hash,verified)VALUES('FilterYearUser','filter-year','year@fixture.invalid','not-a-password',true)").await.unwrap();
    for year in [0,1999,2000,2005,2020,9999i32]{
        let id=format!("FilterYear{year}");let title=format!("Year{year:04}");
        db.execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,demographic,publication_status,content_rating,state)VALUES($1,'FilterYearUser','Fixture',$2,$3,'ja','seinen','ongoing','safe','published')",&[&id,&title,&year]).await.unwrap();
    }
    db.batch_execute("INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,publication_status,content_rating,state)VALUES('YearHidden','FilterYearUser','Fixture','Year hidden',2001,'ja','ongoing','safe','published'),('YearDraft','FilterYearUser','Fixture','Year draft',2001,'ja','ongoing','safe','draft'),('YearDeleted','FilterYearUser','Fixture','Year deleted',2001,'ja','ongoing','safe','published');UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='YearHidden';UPDATE platform.catalog_manga SET deleted_at=clock_timestamp()WHERE id='YearDeleted';INSERT INTO platform.catalog_tags(id,name,tag_group)VALUES('FilterYearTag','Year tag','theme');INSERT INTO platform.catalog_manga_tags(manga_id,tag_id)VALUES('FilterYear2000','FilterYearTag'),('FilterYear2005','FilterYearTag')").await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let tag=resolver.preferred(ResourceKind::Tag,"FilterYearTag").await.unwrap().unwrap();
    // Existing exact wire behavior must pass even on the old-source control.
    for query in ["year=2000","year=none"]{let result=get(&client,&base,&format!("/manga?title=Year&{query}"),200).await;assert_eq!(result["total"],1);}
    for(query,expected)in [("yearFrom=2000",vec![2000,2005,2020,9999]),("yearTo=2000",vec![0,1999,2000]),("yearFrom=0",vec![0,1999,2000,2005,2020,9999]),("yearFrom=0&yearTo=0",vec![0]),("yearFrom=2000&yearTo=2000",vec![2000]),("yearFrom=2001&yearTo=2010",vec![2005]),("yearTo=9999",vec![0,1999,2000,2005,2020,9999])]{
        let path=format!("/manga?title=Year&{query}&order[title]=asc");let count=get(&client,&base,&format!("{path}&limit=0"),200).await;assert_eq!(count["total"],expected.len());assert_eq!(count["data"],json!([]));
        let mut actual=Vec::new();let mut distinct=BTreeSet::new();
        for offset in (0..expected.len()+2).step_by(2){let page=get(&client,&base,&format!("{path}&limit=2&offset={offset}"),200).await;assert_eq!(page["total"],count["total"]);assert_eq!(page["offset"],offset);assert!(page["data"].as_array().unwrap().len()<=2);for row in page["data"].as_array().unwrap(){assert!(distinct.insert(row["id"].as_str().unwrap().to_owned()));actual.push(row["attributes"]["year"].as_i64().unwrap_or(0));}}
        assert_eq!(actual,expected,"{query}");
    }
    let mixed=get(&client,&base,&format!("/manga?title=Year&yearFrom=1999&yearTo=2010&includedTags[]={tag}&originalLanguage[]=ja&status[]=ongoing&contentRating[]=safe&publicationDemographic[]=seinen&limit=1"),200).await;
    assert_eq!(mixed["total"],2);assert_eq!(mixed["data"].as_array().unwrap().len(),1);
    for invalid in ["yearFrom=2001&yearTo=2000","year=2000&yearFrom=2000","year=none&yearTo=0","yearFrom=1&yearFrom=1","yearTo=2&yearTo=3","yearFrom=-1","yearTo=10000","yearFrom=1.5","yearFrom=none","yearFrom=","yearFrom[]=1"]{get(&client,&base,&format!("/manga?title=Year&{invalid}&limit=0"),400).await;get(&client,&base,&format!("/manga?title=Year&{invalid}&limit=1"),400).await;}
}
#[tokio::test(flavor="multi_thread",worker_threads=2)]
#[ignore="Requires explicitly isolated PostgreSQL runner"]
async fn actual_title_groups_keyset_all_pages_languages_visibility_names_and_aliases(){
    let _guard=TEST_LOCK.lock().await;
    let(state,_server,client,base)=fixture().await;let db=state.db.client().await.unwrap();
    db.batch_execute("INSERT INTO platform.users(id,username,email,password_hash,verified)VALUES('FilterGroupUser','filter-group','groups@fixture.invalid','not-a-password',true);INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,year,language,publication_status,content_rating,state)VALUES('FilterGroupTitle','FilterGroupUser','Fixture','Groups across pages',2000,'ja','ongoing','safe','published')").await.unwrap();
    for start in (1..=209i32).step_by(64){let end=(start+63).min(209);
        db.execute("INSERT INTO platform.catalog_groups(id,owner_id,owner_name,name)SELECT 'FiltG'||lpad(n::text,5,'0'),'FilterGroupUser','Fixture','Actual group '||n FROM generate_series($1::integer,$2::integer)n",&[&start,&end]).await.unwrap();
        db.execute("INSERT INTO platform.catalog_chapters(id,manga_id,uploader_id,uploader_name,title,chapter,language,metadata)SELECT 'FilterChapter'||n,'FilterGroupTitle','FilterGroupUser','Fixture','Actual chapter '||n,n::text,CASE WHEN n%2=0 THEN 'ja' ELSE 'en' END,'{\"data\":[{\"name\":\"fixture.webp\",\"dimensions\":[1,1]}]}'::jsonb FROM generate_series($1::integer,$2::integer)n",&[&start,&end]).await.unwrap();
        db.execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id)SELECT 'FilterChapter'||n,'FiltG'||lpad(n::text,5,'0')FROM generate_series($1::integer,$2::integer)n",&[&start,&end]).await.unwrap();
    }
    db.batch_execute("INSERT INTO platform.catalog_chapter_groups(chapter_id,group_id)VALUES('FilterChapter2','FiltG00001');UPDATE platform.catalog_groups SET deleted_at=clock_timestamp()WHERE id='FiltG00206';UPDATE platform.catalog_chapters SET is_unavailable=true WHERE id='FilterChapter207';UPDATE platform.catalog_chapters SET deleted_at=clock_timestamp()WHERE id='FilterChapter208';UPDATE platform.catalog_chapters SET protocol_event_id=repeat('f',64)WHERE id='FilterChapter209'").await.unwrap();
    let resolver=PostgresPublicIdResolver::new(state.db.clone());let title=resolver.preferred(ResourceKind::Manga,"FilterGroupTitle").await.unwrap().unwrap();
    let alias=PublicUuid::parse("aaaaaaaa-aaaa-8aaa-8aaa-aaaaaaaaaaaa").unwrap();resolver.register_alias(ResourceKind::Manga,"FilterGroupTitle",&alias).await.unwrap();
    let expected=db.query("SELECT a.public_id::text,g.name FROM platform.catalog_groups g JOIN platform.public_resource_ids a ON a.kind='group' AND a.native_id=g.id AND a.preferred WHERE g.id BETWEEN 'FiltG00001' AND 'FiltG00205' ORDER BY a.public_id",&[]).await.unwrap().into_iter().map(|row|(row.get::<_,String>(0),row.get::<_,String>(1))).collect::<Vec<_>>();
    // Populate nonempty retained relationship fields for projection equivalence.
    db.batch_execute("INSERT INTO platform.catalog_authors(id,owner_id,owner_name,name,metadata)VALUES('FilterAuthor','FilterGroupUser','Fixture','Actual author','{\"description\":\"Author biography\"}'),('FilterArtist','FilterGroupUser','Fixture','Actual artist','{}');INSERT INTO platform.catalog_manga_creators(manga_id,author_id,role)VALUES('FilterGroupTitle','FilterAuthor','author'),('FilterGroupTitle','FilterArtist','artist');INSERT INTO platform.catalog_tags(id,name,tag_group)VALUES('FilterGroupTag','Actual theme','theme');INSERT INTO platform.catalog_manga_tags(manga_id,tag_id)VALUES('FilterGroupTitle','FilterGroupTag');INSERT INTO platform.catalog_covers(id,manga_id,uploader_id,uploader_name,ext,dimensions,is_main,language,description)VALUES('FilterCover','FilterGroupTitle','FilterGroupUser','Fixture','webp',ARRAY[2,3],true,'ja','Actual cover');INSERT INTO platform.catalog_manga(id,owner_id,owner_name,title,language,publication_status,content_rating,state)VALUES('FilterRelatedTitle','FilterGroupUser','Fixture','Related projection title','ja','ongoing','safe','published');INSERT INTO platform.catalog_relations(manga_id,related_id,relation_type,created_by)VALUES('FilterGroupTitle','FilterRelatedTitle','sequel','FilterGroupUser')").await.unwrap();
    // Preserve all native projected metadata, source fields and nested nulls.
    db.batch_execute(r#"UPDATE platform.catalog_manga SET metadata=metadata || '{"description":{"en":"Projection description"},"links":{"raw":"https://fixture.invalid/source"},"protocol_event_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","protocol_publisher":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","publication_scope":"original-protocol","publication_status":"published","instance_overlay":true,"nullable_field":null,"nested":{"absent":null,"present":"kept"},"nullable_array":[null,{"absent":null}]}'::jsonb WHERE id='FilterGroupTitle';UPDATE platform.catalog_chapters SET source_id='fixture-source-chapter',volume=NULL,metadata=metadata || '{"null_property":null,"protocol_publisher":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","nested":{"absent":null,"present":"kept"},"nullable_array":[null,{"absent":null}]}'::jsonb WHERE id='FilterChapter1'"#).await.unwrap();
    assert_projection_equivalence(&state,&client,&base,"FilterGroupTitle","FilterChapter1").await;
    let aliases_before:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);
    let detail=get(&client,&base,&format!("/manga/{title}"),200).await;assert_eq!(detail["data"]["attributes"]["nostr"]["availableGroupSemantics"],"visible-title-groups-v1");
    // Never construct legacy unbounded group/member JSON in the title read.
    let projection:Value=db.query_one("SELECT platform.catalog_public_manga_json('FilterGroupTitle')",&[]).await.unwrap().get(0);
    assert!(projection["relationships"].get("available_groups").is_none());
    let mut actual=Vec::new();let mut after=None;let mut pages=0;
    loop{let path=format!("/manga/{alias}/available-groups?limit=100{}",after.as_ref().map(|id|format!("&after={id}")).unwrap_or_default());let page=get(&client,&base,&path,200).await;pages+=1;assert_eq!(page["total"],205);assert_eq!(page["position"],actual.len());assert!(page["data"].as_array().unwrap().len()<=100);for row in page["data"].as_array().unwrap(){let id=row["id"].as_str().unwrap();assert!(PublicUuid::parse(id).is_ok());assert_eq!(row["type"],"scanlation_group");assert_eq!(row["relationships"],json!([]));assert_eq!(row["attributes"].as_object().unwrap().len(),1);actual.push((id.to_owned(),row["attributes"]["name"].as_str().unwrap().to_owned()));}after=page["next"].as_str().map(str::to_owned);if after.is_none(){break}assert_eq!(after.as_deref(),actual.last().map(|row|row.0.as_str()));assert!(pages<=3);}
    assert_eq!(pages,3);assert_eq!(actual,expected);
    let selected=&expected[204].0;let hydrated=get(&client,&base,&format!("/manga/{title}/available-groups?ids[]={selected}"),200).await;assert_eq!(hydrated["total"],1);assert_eq!(hydrated["position"],0);assert_eq!(hydrated["data"][0]["attributes"]["name"],expected[204].1);assert_eq!(hydrated["next"],Value::Null);
    // Existing UUID chapter filtering and current page/language cannot shrink options.
    let chapter=get(&client,&base,&format!("/chapter?manga={title}&groups[]={selected}&includes=scanlation_group&limit=1"),200).await;assert!(chapter["total"].as_u64().unwrap()>0);
    get(&client,&base,&format!("/manga/{title}/feed?translatedLanguage[]=en&limit=1"),200).await;
    assert_eq!(get(&client,&base,&format!("/manga/{title}/available-groups?limit=1"),200).await["total"],205);
    for invalid in ["limit=0","limit=101","limit=2&limit=2","offset=1","after=bad","translatedLanguage[]=en"]{get(&client,&base,&format!("/manga/{title}/available-groups?{invalid}"),400).await;}
    get(&client,&base,&format!("/manga/{title}/available-groups?after={selected}&ids[]={selected}"),400).await;
    // Withdrawal of the cursor is explicit conflict, not a false final page.
    db.execute("UPDATE platform.catalog_groups SET deleted_at=clock_timestamp()WHERE id=(SELECT native_id FROM platform.public_resource_ids WHERE kind='group'AND public_id=$1::text::uuid)",&[&selected]).await.unwrap();
    get(&client,&base,&format!("/manga/{title}/available-groups?after={selected}"),409).await;
    let changed=get(&client,&base,&format!("/manga/{title}/available-groups?limit=100"),200).await;assert_eq!(changed["total"],204);assert!(!ids(&changed).contains(selected));
    // Optional group failure never makes a legitimate large title unavailable.
    assert_eq!(get(&client,&base,&format!("/manga/{title}"),200).await["data"]["id"],title.as_str());
    db.batch_execute("UPDATE platform.catalog_manga SET protocol_source_visible=false WHERE id='FilterGroupTitle'").await.unwrap();
    get(&client,&base,&format!("/manga/{title}/available-groups"),404).await;
    let hidden=db.query_one("SELECT platform.catalog_chapter_json('FilterChapter1',platform.catalog_public_manga_json('FilterGroupTitle'),'[]'::jsonb),platform.catalog_public_chapter_json('FilterChapter1')",&[]).await.unwrap();
    assert!(hidden.get::<_,Option<Value>>(0).is_none());assert!(hidden.get::<_,Option<Value>>(1).is_none());
    let aliases_after:i64=db.query_one("SELECT count(*) FROM platform.public_resource_ids",&[]).await.unwrap().get(0);assert_eq!(aliases_before,aliases_after);
}
// Deliberately compare finite fixture values using the installed latest native
// functions, not a textual0082 oracle. Only the declared presentation extension
// is removed; every other value (including metadata/nulls) must remain equal.
fn remove_group_extension(value:&mut Value){
    match value{
        Value::Object(object)=>{
            object.remove("_public_title_groups");
            if let Some(Value::Object(relationships))=object.get_mut("relationships"){relationships.remove("available_groups");}
            if let Some(Value::Object(nostr))=object.get_mut("nostr"){nostr.remove("availableGroupSemantics");}
            for child in object.values_mut(){remove_group_extension(child);}
        },
        Value::Array(array)=>for child in array{remove_group_extension(child)},_=>{}
    }
}
async fn assert_projection_equivalence(state:&AppState,client:&reqwest::Client,base:&str,manga:&str,chapter:&str){
    use weebdex_platform::public_api::catalogue_dto::AliasIndex;
    let db=state.db.client().await.unwrap();
    let row=db.query_one("SELECT platform.catalog_manga_json($1),platform.catalog_public_manga_json($1),platform.catalog_chapter_json($2),platform.catalog_chapter_json($2,platform.catalog_manga_json($1),(platform.catalog_chapter_json($2)->'relationships'->'groups')),platform.catalog_public_chapter_json($2),platform.catalog_chapter_json($2,NULL::jsonb,NULL::jsonb),(SELECT id FROM platform.catalog_chapters WHERE manga_id=$1 AND deleted_at IS NULL AND platform.protocol_source_is_visible(protocol_event_id) ORDER BY published_at DESC,id LIMIT 1)",&[&manga,&chapter]).await.unwrap();
    let mut native_title:Value=row.get(0);let mut public_title:Value=row.get(1);let native_chapter:Value=row.get(2);let latest_chapter:Value=row.get(3);let public_chapter:Value=row.get(4);let null_relationships:Value=row.get(5);
    assert_eq!(native_chapter,latest_chapter,"latest0088 retains the complete one-argument canonical value");
    let mut expected_nulls=native_chapter.clone();expected_nulls["relationships"].as_object_mut().unwrap().remove("manga");expected_nulls["relationships"].as_object_mut().unwrap().remove("groups");assert_eq!(null_relationships,expected_nulls,"0088 recursively strips null object properties but retains array nulls");
    for value in [&native_chapter,&public_chapter]{assert!(value.get("null_property").is_none());assert!(value["nested"].get("absent").is_none());assert_eq!(value["nullable_array"],json!([null,{}]));assert_eq!(value["source_id"],"fixture-source-chapter");assert_eq!(value["protocol_publisher"],"b".repeat(64));}
    for(a,b)in[(&native_title,&public_title),(&latest_chapter,&public_chapter)]{let(mut a,mut b)=(a.clone(),b.clone());remove_group_extension(&mut a);remove_group_extension(&mut b);assert_eq!(a,b,"all retained projection fields must be equivalent");}
    let latest:Option<String>=row.get(6);if let Some(latest)=latest{native_title["_latest_chapter_id"]=json!(latest);public_title["_latest_chapter_id"]=native_title["_latest_chapter_id"].clone();}
    let resolver=PostgresPublicIdResolver::new(state.db.clone());
    for(kind,native,public,includes,path)in[(ResourceKind::Manga,native_title,public_title,BTreeSet::new(),"manga"),(ResourceKind::Chapter,latest_chapter,public_chapter,BTreeSet::from(["manga".to_owned(),"scanlation_group".to_owned(),"user".to_owned()]),"chapter")]{
        let index=AliasIndex::load(state,&resolver,&[(kind,native.clone()),(kind,public.clone())]).await.unwrap();
        let old=index.resource(kind,&native,&includes).unwrap();let current=index.resource(kind,&public,&includes).unwrap();
        let(mut old_value,mut current_value)=(serde_json::to_value(&old).unwrap(),serde_json::to_value(&current).unwrap());remove_group_extension(&mut old_value);remove_group_extension(&mut current_value);assert_eq!(old_value,current_value,"canonical DTO fields and provenance unchanged");
        let query=includes.iter().map(|include|format!("includes[]={include}")).collect::<Vec<_>>().join("&");let http=get(client,base,&format!("/{path}/{}?{query}",current.id),200).await;assert_eq!(http["data"],json!(current),"actual HTTP is the equivalent latest-native projection");
    }
    // Missing, source-withdrawn and deleted chapters keep latest-native NULL.
    for id in ["MissingFilterChapter","FilterChapter208","FilterChapter209"]{let row=db.query_one("SELECT platform.catalog_chapter_json($1,platform.catalog_public_manga_json($2),'[]'::jsonb),platform.catalog_public_chapter_json($1)",&[&id,&manga]).await.unwrap();let native:Option<Value>=row.get(0);let public:Option<Value>=row.get(1);assert_eq!(native,public);assert!(public.is_none());}
}
