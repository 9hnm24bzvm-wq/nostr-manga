//! Scale-oriented node foundation. PostgreSQL is authoritative; Moka is derived.
//! SQLx and SQLite are intentionally absent from this crate.
pub mod auth;
pub mod db;

pub mod manifest;

pub mod capacity;

pub mod node;

pub mod storage;

pub mod blobs;

pub mod upload;

pub mod backend;

pub mod publication;

pub mod filter;

pub mod discovery;

pub mod moderation;

pub mod transfer;

pub mod peers;

pub mod inventory;

pub mod sync_jobs;

pub mod groups;

pub mod library_manifest;

pub mod accounts;

pub mod library_storage;

pub mod health;

mod storage_accounting;

pub mod library;

mod library_references;

mod bounded_response;
pub mod library_wire;

pub mod library_admission;

mod perf;

mod reclamation;

mod library_history;

pub mod social;

pub mod ordinary_deletion;

pub mod relay_feed;
pub mod relay_query;

pub mod relay;

pub mod sync_schedules;

pub mod service_proof;

pub mod public_exports;

pub mod serving;

pub mod export_queue;

pub mod serving_role;

pub mod publisher_authority;

pub mod staging_operation;

pub mod staged_deletion;

pub mod transfer_rejection;

pub mod capabilities;
pub mod protocol_limits;

pub mod blossom;

pub mod offline_keys;

pub mod background_workers;

pub mod endpoint_policy;

pub mod service_identity;

pub mod service_announcement;

pub mod library_client_bundle;

pub mod library_client;

pub mod library_client_recovery;

pub mod library_client_outbox;

pub mod library_client_relays;

pub mod submission_review;

pub mod library_client_discovery;

pub mod peer_discovery;

pub mod series_metadata;

pub mod catalogue;

pub mod mangadex_compat;

pub mod compatibility_query;

pub mod library_view;

pub mod enrollment;

pub mod download_budget;

pub mod read_admission;
mod read_diagnostics;

mod storage_admission;

pub mod relay_dispatch;

/// Read-only catalogue seam for downstream instance-local projections.
pub mod platform_bridge;
