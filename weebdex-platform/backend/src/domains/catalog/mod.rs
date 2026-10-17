mod derivatives;
pub(crate) mod entity_mutations;
mod feed_diagnostics;
mod feed_projection;
pub mod languages;
pub(crate) mod mutations;
mod protocol_bridge;
pub mod query;
mod read;
pub mod repository;
pub mod validation;
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, post},
};
pub use repository::{CatalogPage, CatalogRepository, ChapterPublication, CoverPublication};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/manga",
            get(read::manga_search).post(mutations::manga_create),
        )
        .route("/manga/random", get(read::manga_random))
        .route("/manga/top", get(read::manga_top))
        .route("/manga/tag", get(read::tag_search))
        .route("/manga/import-mappings", post(read::manga_import_mappings))
        .route("/manga/draft", get(read::draft_search))
        .route("/manga/draft/{id}", get(read::draft_get))
        .route("/manga/draft/{id}/commit", post(mutations::manga_commit))
        .route("/manga/draft/{id}/publish", post(mutations::manga_publish))
        .route("/manga/draft/{id}/reject", post(mutations::manga_reject))
        .route(
            "/manga/{id}",
            get(read::manga_get)
                .put(mutations::manga_update)
                .delete(mutations::manga_delete),
        )
        .route(
            "/manga/{id}/relations",
            get(read::manga_relations)
                .post(mutations::relation_create)
                .put(mutations::relation_update)
                .delete(mutations::relation_delete),
        )
        .route(
            "/manga/{id}/recommendations",
            get(read::manga_recommendations),
        )
        .route("/manga/{id}/chapters", get(read::chapter_list))
        .route("/manga/{id}/aggregate", get(read::chapter_aggregate))
        .route("/manga/{id}/covers", get(read::cover_list))
        .route("/manga/{id}/lock", post(mutations::manga_lock))
        .route("/manga/{id}/unlock", post(mutations::manga_unlock))
        .route("/chapter", get(read::chapter_search))
        .route("/chapter/updates", get(read::chapter_updates))
        .route("/chapter/feed", get(read::chapter_latest))
        .route("/chapter/top", get(read::chapter_top))
        .route("/chapter/{id}", get(read::chapter_get))
        .route(
            "/author",
            get(read::author_search).post(mutations::author_create),
        )
        .route(
            "/author/{id}",
            get(read::author_get)
                .put(mutations::author_update)
                .delete(mutations::author_delete),
        )
        .route(
            "/group",
            get(read::group_search).post(mutations::group_create),
        )
        .route(
            "/group/{id}",
            get(read::group_get)
                .put(mutations::group_update)
                .delete(mutations::group_delete),
        )
}
