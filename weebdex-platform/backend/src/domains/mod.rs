use crate::{db::Migration, state::AppState};
use axum::Router;
pub mod analytics;
pub mod announcements;
pub mod auth;
pub mod catalog;
pub mod community;
pub mod library;
pub mod moderation;
pub mod previews;
pub mod uploads;
pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(auth::api_router())
        .merge(catalog::router())
        .merge(uploads::router())
        .merge(library::router())
        .merge(community::router())
        .merge(moderation::router())
        .merge(analytics::router())
        .merge(announcements::router())
        .merge(crate::protocol::adapters::router())
}
pub fn auth_router() -> Router<AppState> {
    auth::router()
}
pub fn media_router() -> Router<AppState> {
    uploads::media_router()
}
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "0001_catalog_people",
        sql: include_str!("../../migrations/0001_catalog_people.sql"),
    },
    Migration {
        version: 2,
        name: "0002_catalog_manga",
        sql: include_str!("../../migrations/0002_catalog_manga.sql"),
    },
    Migration {
        version: 3,
        name: "0003_catalog_chapters",
        sql: include_str!("../../migrations/0003_catalog_chapters.sql"),
    },
    Migration {
        version: 4,
        name: "0004_catalog_covers",
        sql: include_str!("../../migrations/0004_catalog_covers.sql"),
    },
    Migration {
        version: 5,
        name: "0005_catalog_relations",
        sql: include_str!("../../migrations/0005_catalog_relations.sql"),
    },
    Migration {
        version: 6,
        name: "0006_catalog_discussions",
        sql: include_str!("../../migrations/0006_catalog_discussions.sql"),
    },
    Migration {
        version: 7,
        name: "0007_catalog_counters",
        sql: include_str!("../../migrations/0007_catalog_counters.sql"),
    },
    Migration {
        version: 8,
        name: "0008_catalog_audit",
        sql: include_str!("../../migrations/0008_catalog_audit.sql"),
    },
    Migration {
        version: 9,
        name: "0009_catalog_projections",
        sql: include_str!("../../migrations/0009_catalog_projections.sql"),
    },
    Migration {
        version: 10,
        name: "0010_native_auth",
        sql: include_str!("../../migrations/0010_native_auth.sql"),
    },
    Migration {
        version: 11,
        name: "0011_api_clients",
        sql: include_str!("../../migrations/0011_api_clients.sql"),
    },
    Migration {
        version: 12,
        name: "0012_email_tokens",
        sql: include_str!("../../migrations/0012_email_tokens.sql"),
    },
    Migration {
        version: 20,
        name: "0020_upload_sessions",
        sql: include_str!("../../migrations/0020_upload_sessions.sql"),
    },
    Migration {
        version: 21,
        name: "0021_upload_objects",
        sql: include_str!("../../migrations/0021_upload_objects.sql"),
    },
    Migration {
        version: 22,
        name: "0022_chapter_media",
        sql: include_str!("../../migrations/0022_chapter_media.sql"),
    },
    Migration {
        version: 23,
        name: "0023_chapter_derivatives",
        sql: include_str!("../../migrations/0023_chapter_derivatives.sql"),
    },
    Migration {
        version: 30,
        name: "0030_library",
        sql: include_str!("../../migrations/0030_library.sql"),
    },
    Migration {
        version: 40,
        name: "0040_community",
        sql: include_str!("../../migrations/0040_community.sql"),
    },
    Migration {
        version: 50,
        name: "0050_moderation",
        sql: include_str!("../../migrations/0050_moderation.sql"),
    },
    Migration {
        version: 60,
        name: "0060_analytics",
        sql: include_str!("../../migrations/0060_analytics.sql"),
    },
    Migration {
        version: 70,
        name: "0070_protocol_bridge",
        sql: include_str!("../../migrations/0070_protocol_bridge.sql"),
    },
    Migration {
        version: 71,
        name: "0071_protocol_deletion_intents",
        sql: include_str!("../../migrations/0071_protocol_deletion_intents.sql"),
    },
    Migration {
        version: 72,
        name: "0072_upload_resume",
        sql: include_str!("../../migrations/0072_upload_resume.sql"),
    },
    Migration {
        version: 73,
        name: "0073_catalogue_namespace",
        sql: include_str!("../../migrations/0073_catalogue_namespace.sql"),
    },
    Migration {
        version: 74,
        name: "0074_protocol_command_inputs",
        sql: include_str!("../../migrations/0074_protocol_command_inputs.sql"),
    },
    Migration {
        version: 80,
        name: "0080_announcements",
        sql: include_str!("../../migrations/0080_announcements.sql"),
    },
    Migration {
        version: 81,
        name: "0081_community_visibility",
        sql: include_str!("../../migrations/0081_community_visibility.sql"),
    },
    Migration {
        version: 82,
        name: "0082_catalog_projection_visibility",
        sql: include_str!("../../migrations/0082_catalog_projection_visibility.sql"),
    },
    Migration {
        version: 83,
        name: "0083_upload_maintenance_indexes",
        sql: include_str!("../../migrations/0083_upload_maintenance_indexes.sql"),
    },
    Migration {
        version: 84,
        name: "0084_protocol_deletion_generations",
        sql: include_str!("../../migrations/0084_protocol_deletion_generations.sql"),
    },
    Migration {
        version: 85,
        name: "0085_cover_extension_contract",
        sql: include_str!("../../migrations/0085_cover_extension_contract.sql"),
    },
    Migration {
        version: 86,
        name: "0086_visibility_policy_preparation",
        sql: include_str!("../../migrations/0086_visibility_policy_preparation.sql"),
    },
    Migration {
        version: 87,
        name: "0087_catalog_chapter_relationship_projection",
        sql: include_str!("../../migrations/0087_catalog_chapter_relationship_projection.sql"),
    },
    Migration {
        version: 88,
        name: "0088_catalog_correlated_visibility_lookups",
        sql: include_str!("../../migrations/0088_catalog_correlated_visibility_lookups.sql"),
    },
    Migration {
        version: 89,
        name: "0089_public_resource_ids",
        sql: include_str!("../../migrations/0089_public_resource_ids.sql"),
    },
    Migration { version:90,name:"0090_public_manga_edge_ids",sql:include_str!("../../migrations/0090_public_manga_edge_ids.sql") },
    Migration { version: 91, name: "0091_protocol_public_list_aliases", sql: include_str!("../../migrations/0091_protocol_public_list_aliases.sql") },
    Migration {version:92,name:"0092_public_report_reasons",sql:include_str!("../../migrations/0092_public_report_reasons.sql")},
    Migration {version:94,name:"0094_cached_parent_visibility",sql:include_str!("../../migrations/0094_cached_parent_visibility.sql")},
    Migration {version:97,name:"0097_chapter_removal_review",sql:include_str!("../../migrations/0097_chapter_removal_review.sql")},
    Migration {version:98,name:"0098_native_manga_ratings",sql:include_str!("../../migrations/0098_native_manga_ratings.sql")},
    Migration {version:99,name:"0099_native_public_list_follows",sql:include_str!("../../migrations/0099_native_public_list_follows.sql")},
    Migration {version:100,name:"0100_native_follow_revisions",sql:include_str!("../../migrations/0100_native_follow_revisions.sql")},
    Migration {version:101,name:"0101_title_removal_review",sql:include_str!("../../migrations/0101_title_removal_review.sql")},
    Migration {version:102,name:"0102_native_reading_status_revisions",sql:include_str!("../../migrations/0102_native_reading_status_revisions.sql")},
    Migration {version:103,name:"0103_scheduled_uploads",sql:include_str!("../../migrations/0103_scheduled_uploads.sql")},
    Migration {version:104,name:"0104_public_title_filter_groups",sql:include_str!("../../migrations/0104_public_title_filter_groups.sql")},
    Migration {version:105,name:"0105_account_disable",sql:include_str!("../../migrations/0105_account_disable.sql")},
    Migration {version:106,name:"0106_native_account_library_revision",sql:include_str!("../../migrations/0106_native_account_library_revision.sql")},
];
