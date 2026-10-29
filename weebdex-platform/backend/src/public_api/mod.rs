//! Versioned public adapter boundary, reconstructed after execution workspace loss.
//! This initial source checkpoint does not claim endpoint coverage.
pub mod at_home_core;
pub mod cover_filename;
pub mod dto;
mod title_links;
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
        .merge(native_account::router())
        .merge(native_account_profile_settings::router())
        .merge(native_contributions::router())
        .merge(native_drafts::router())
        .merge(native_uploads::router())
        .merge(scheduled_uploads::native_router())
        .merge(native_upload_extensions::router())
        .merge(native_cover_writes::router())
        .merge(native_editorial::router())
        .merge(native_relation_writes::router())
        .merge(native_title_disposition::router())
        .merge(native_chapter_writes::router())
        .merge(native_account_library::router())
        .merge(library::router())
        .merge(lists::router())
        .merge(profiles::router())
        .merge(reports::router())
        .merge(statistics::router())
        .merge(popularity::router())
        .merge(catalogue::router())
        .merge(reader_selector::router())
        .merge(catalogue_edges::router())
        .merge(account_reads::router())
        .merge(account_reads::native_router())
        .merge(ratings::router())
        .merge(list_follows::router())
        .merge(list_follows::native_router())
        .merge(follow_writes::router())
        .merge(native_reading_status::router())
        .merge(native_profile::router())
        .merge(account_closure::router())
        .merge(native_user_memberships::router())
        .merge(native_user_discussion::router())
        .merge(native_discussion::router())
        .merge(native_discussion_reports::router())
        .merge(native_user_reports::router())
        .merge(native_report_inspection::router())
        .merge(moderator_reports::router())
        .merge(media::router())
        .merge(native_forum::router())
        .merge(legacy::router())
        .merge(statuses::router())
        .merge(editorial::router())
        .merge(drafts::router())
        .merge(uploads::router())
        .merge(upload_extensions::router())
        .merge(scheduled_uploads::router())
        .merge(chapter_writes::router())
        .merge(manga_writes::router())
        .merge(cover_writes::router())
        .merge(title_disposition::router())
        .merge(resource_mappings::router())
        .merge(relation_writes::router())
        .method_not_allowed_fallback(routes::wrong_method)
}

pub mod auth;
mod native_account;
mod native_account_profile_settings;
mod native_contributions;
mod native_json;
mod native_drafts;
mod native_uploads;
mod native_upload_extensions;
mod native_cover_writes;
mod native_editorial;
mod native_relation_writes;
mod native_title_disposition;
mod native_chapter_writes;
mod native_account_library;

pub mod catalogue_dto;

pub mod edge_identity;
pub mod library;
pub mod lists;
pub mod profiles;

pub mod reports;

pub mod statistics;
pub mod popularity;

pub mod catalogue;
pub mod reader_selector;
pub mod catalogue_edges;
pub mod account_reads;
pub mod account_settings;
pub mod settings_writes;
pub mod ratings;
pub mod list_follows;
pub mod follow_writes;
pub mod native_reading_status;
pub mod native_profile;
pub mod native_user_memberships;
mod native_discussion_common;
pub mod native_discussion;
mod native_discussion_reports;
pub mod account_closure;
pub mod native_user_discussion;
pub mod native_user_reports;
pub mod native_report_inspection;
pub mod moderator_reports;

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

mod publish_at;
pub mod scheduled_uploads;
