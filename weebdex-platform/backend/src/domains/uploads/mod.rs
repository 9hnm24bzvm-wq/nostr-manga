//! Instance-owned streamed image uploads. No unsigned record impersonates protocol authority.
mod derivatives;
pub(crate) mod media;
mod proxy;
mod sessions;
pub mod storage;
use crate::{db::Migration, state::AppState};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
};
pub use proxy::{ProxyQuery, proxy};

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 20,
        name: "upload_sessions",
        sql: include_str!("../../../migrations/0020_upload_sessions.sql"),
    },
    Migration {
        version: 21,
        name: "upload_objects",
        sql: include_str!("../../../migrations/0021_upload_objects.sql"),
    },
    Migration {
        version: 22,
        name: "chapter_media",
        sql: include_str!("../../../migrations/0022_chapter_media.sql"),
    },
    Migration {
        version: 23,
        name: "chapter_derivatives",
        sql: include_str!("../../../migrations/0023_chapter_derivatives.sql"),
    },
];
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/upload", get(sessions::get))
        .route("/upload/begin", post(sessions::begin))
        .route("/upload/begin/{chapter}", post(sessions::begin_edit))
        .route(
            "/upload/check-approval-required",
            post(sessions::check_approval),
        )
        .route(
            "/upload/{session}",
            delete(sessions::cancel)
                .post(sessions::upload)
                .patch(sessions::save_resume)
                .layer(DefaultBodyLimit::max(96 * 1024 * 1024)),
        )
        .route(
            "/upload/{session}/files/{file}/replace",
            post(sessions::replace_file).layer(DefaultBodyLimit::max(40 * 1024 * 1024)),
        )
        .route("/upload/{session}/commit", post(sessions::commit))
        .route("/upload/{session}/batch", delete(sessions::delete_files))
        .route("/upload/{session}/{file}", delete(sessions::delete_file))
        .route(
            "/cover/{id}",
            get(sessions::get_cover)
                .post(sessions::upload_cover)
                .put(sessions::update_cover)
                .delete(sessions::delete_cover)
                .layer(DefaultBodyLimit::max(40 * 1024 * 1024)),
        )
        .route(
            "/chapter/{id}",
            put(sessions::update_chapter).delete(sessions::delete_chapter),
        )
        .route("/chapter/{id}/claim", post(sessions::claim_chapter))
}
pub fn media_router() -> Router<AppState> {
    Router::new()
        .route(
            "/data/{chapter}/{name}",
            get(media::chapter).head(media::chapter),
        )
        .route(
            "/data-saver/{chapter}/{name}",
            get(media::chapter).head(media::chapter),
        )
        .route("/uploads/{name}", get(media::preview).head(media::preview))
        .route(
            "/covers/{manga}/{name}",
            get(media::cover).head(media::cover),
        )
}

/// Call after schema verification before serving; ordinary upload mutations also retry durable cleanup.
pub async fn recover_storage(state: &AppState) {
    sessions::cleanup(state).await;
    crate::runtime::cleanup_expired_temporary(&state.config.uploads_dir).await;
}
