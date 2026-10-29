//! Native Rust compatibility for the unchanged @ory/client browser flow API.
//! No Kratos service, dummy session, or unverified email trust is used.
mod clients;
pub(crate) mod closure;
mod crypto;
mod flows;
mod mail;
mod preferences;
mod principal;
pub use mail::MailConfig;

use crate::{db::Migration, state::AppState};
use axum::{Router, routing::get};
pub use principal::{
    SessionPrincipal, optional_personal_principal, optional_principal, require_library_principal,
    require_principal, validate_mutation,
};
pub(crate) use principal::require_native_library_principal_in;
pub(crate) use principal::require_native_account_principal_in;

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 10,
        name: "native_auth",
        sql: include_str!("../../../migrations/0010_native_auth.sql"),
    },
    Migration {
        version: 11,
        name: "api_clients",
        sql: include_str!("../../../migrations/0011_api_clients.sql"),
    },
    Migration {
        version: 12,
        name: "email_tokens",
        sql: include_str!("../../../migrations/0012_email_tokens.sql"),
    },
];

pub fn router() -> Router<AppState> {
    flows::router()
}

pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/auth/check", get(preferences::check))
        .merge(clients::router())
        .merge(closure::router())
}
