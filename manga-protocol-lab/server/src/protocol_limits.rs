//! Shared enforced and advertised protocol budgets.
pub const WS_MESSAGE_BYTES: usize = 1_048_576;
pub const RELAY_SUBSCRIPTIONS: usize = 8;
pub const RELAY_FILTERS: usize = 8;
pub const RELAY_SUBID_BYTES: usize = 64;
pub const RELAY_LIMIT: i64 = 500;
pub const QUERY_BYTES: usize = 32768;
pub const QUERY_LIMIT: i64 = 1000;
pub const FILTER_BYTES: usize = 8192;
pub const FILTER_TOKENS: usize = 500;
pub const FILTER_DEPTH: usize = 20;
pub const FILTER_FIELDS: &[&str] = &[
    "series",
    "publisher",
    "language",
    "pages",
    "size",
    "created_at",
    "chapter",
    "volume",
    "title",
];
