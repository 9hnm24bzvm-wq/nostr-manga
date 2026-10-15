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
pub use errors::{PublicApiError,PublicApiResult};
pub use id::PublicUuid;
pub use identity::{PublicIdResolver,PublicIdTarget,ResolvedPublicId,ResourceKind};

pub mod postgres_identity;
pub use postgres_identity::PostgresPublicIdResolver;
