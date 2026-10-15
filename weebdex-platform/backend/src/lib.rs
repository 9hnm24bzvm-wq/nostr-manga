//! WeebDex's presentation and instance workflows, backed by manga-node.
//! The protocol source and its migration history remain independent.

pub mod client_peer;
pub mod config;
pub mod contracts;
pub mod db;
pub mod domains;
pub mod error;
pub mod privacy;
pub mod protocol;
pub mod public_api;
pub mod router;
pub mod runtime;
pub mod state;

pub use config::Config;
pub use error::{ApiError, ApiResult};
pub use state::AppState;
