//! Versioned public adapter boundary, reconstructed after execution workspace loss.
//! This initial source checkpoint does not claim endpoint coverage.
pub mod at_home_core;
pub mod cover_filename;
pub mod dto;
pub mod errors;
pub mod id;
pub mod identity;
pub mod native_forum_core;
pub mod query;
pub mod statuses_core;
pub use errors::{PublicApiError, PublicApiResult};
pub use id::PublicUuid;
pub use identity::{PublicIdResolver, PublicIdTarget, ResolvedPublicId, ResourceKind};

pub mod postgres_identity;
pub use postgres_identity::PostgresPublicIdResolver;

pub mod routes;
pub fn router() -> axum::Router<crate::AppState> {
    routes::router()
        .merge(auth::router())
        .merge(library::router())
        .merge(lists::router())
        .merge(profiles::router())
        .merge(reports::router())
        .merge(statistics::router())
        .merge(catalogue::router())
        .merge(catalogue_edges::router())
        .merge(account_reads::router())
        .merge(ratings::router())
        .merge(list_follows::router())
        .merge(follow_writes::router())
        .merge(native_reading_status::router())
        .merge(media::router())
        .merge(native_forum::router())
        .merge(legacy::router())
        .merge(statuses::router())
        .merge(editorial::router())
        .merge(drafts::router())
        .merge(uploads::router())
        .merge(upload_extensions::router())
        .merge(chapter_writes::router())
        .merge(manga_writes::router())
        .merge(cover_writes::router())
        .merge(title_disposition::router())
        .merge(resource_mappings::router())
        .merge(relation_writes::router())
        .method_not_allowed_fallback(routes::wrong_method)
}

pub mod auth;

pub mod catalogue_dto;

pub mod edge_identity;
pub mod library;
pub mod lists;
pub mod profiles;

pub mod reports;

pub mod statistics;

pub mod catalogue;
pub mod catalogue_edges;
pub mod account_reads;
pub mod account_settings;
pub mod settings_writes;
pub mod ratings;
pub mod list_follows;
pub mod follow_writes;
pub mod native_reading_status;

pub mod media;

pub mod native_forum;

pub mod legacy;

pub mod statuses;
pub mod drafts;
pub mod editorial;

pub mod uploads;
pub mod upload_extensions;
mod uploads_body;

pub mod chapter_writes;

pub mod manga_writes;

pub mod cover_writes;

pub mod title_disposition;
pub mod resource_mappings;
pub mod relation_writes;
pub(crate) mod relation_paging;
