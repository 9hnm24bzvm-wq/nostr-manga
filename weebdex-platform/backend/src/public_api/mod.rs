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
        .merge(media::router())
        .merge(native_forum::router())
        .merge(legacy::router())
        .merge(statuses::router())
        .merge(editorial::router())
        .merge(drafts::router())
        .merge(uploads::router())
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

pub mod media;

pub mod native_forum;

pub mod legacy;

pub mod statuses;
pub mod drafts;
pub mod editorial;

pub mod uploads;
mod uploads_body;
