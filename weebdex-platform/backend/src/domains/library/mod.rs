//! WeebDex instance account library. Browser-local protocol libraries and opted-in
//! signed Nostr library publication remain separate and unchanged.
mod follows;
mod history;
mod lists;
mod preferences;
pub(crate) mod preference_core;
mod progress;
mod query;
mod repository;
mod users;
mod validation;
use crate::{db::Migration, state::AppState};
use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
    routing::{get, post, put},
    Router,
};
pub use repository::session_projection;
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 30,
    name: "library",
    sql: include_str!("../../../migrations/0030_library.sql"),
}];
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings", put(preferences::update))
        .route("/user", get(users::search))
        .route("/user/follows/manga", get(follows::search))
        .route("/user/follows/manga/import", post(follows::import))
        .route("/user/follows/manga/export", get(follows::export))
        .route(
            "/user/follows/manga/{id}",
            post(follows::update).delete(follows::unfollow),
        )
        .route("/user/follows/chapter/feed", get(follows::chapter_feed))
        .route(
            "/user/follows/chapter/updates",
            get(follows::chapter_updates),
        )
        .route("/user/follows/users", get(follows::users))
        .route(
            "/user/follows/users/{id}",
            get(follows::user_status)
                .post(follows::user_follow)
                .delete(follows::user_unfollow),
        )
        .route("/user/follows/groups", get(follows::groups))
        .route(
            "/user/follows/groups/{id}",
            get(follows::group_status)
                .post(follows::group_follow)
                .delete(follows::group_unfollow),
        )
        .route("/user/history/manga", get(history::search))
        .route("/user/history/manga/feed", get(history::feed))
        .route(
            "/user/history/manga/{id}",
            get(history::list).post(history::update),
        )
        .route(
            "/user/history/chapter/{id}/progress",
            get(progress::get).put(progress::update),
        )
        .route("/user/lists", get(lists::mine).post(lists::create))
        .route("/user/{id}/lists", get(lists::by_user))
        .route(
            "/list/{id}",
            get(lists::get).put(lists::update).delete(lists::delete),
        )
        .route("/user/{id}", get(users::get).put(users::update))
        .layer(axum::middleware::from_fn(private_headers))
}
async fn private_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response.headers_mut().insert(
        header::VARY,
        HeaderValue::from_static("Cookie, Authorization"),
    );
    response
}

pub use repository::public_user_profile;
