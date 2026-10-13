//! Pure storage seams require no database or user credentials.
use weebdex_platform::domains::uploads::storage;
use std::path::Path;
#[test]
fn uploaded_filename_ids_match_unchanged_frontend_commit_guard() {
 for _ in 0..1000 {let id=storage::short_id();assert_eq!(id.len(),10);assert!(storage::valid_id(&id));}
}
#[test]
fn arbitrary_client_paths_never_address_original_storage() {
 for input in ["../outside", "..", "/etc/passwd", "file:///etc/passwd", "a%2fb", "a\\b", "secret.key"] {
  assert!(storage::object_path(Path::new("/instance/uploads"),input).is_err());
 }
}
#[test]
fn original_decode_budgets_match_server_enforcement() {
 assert_eq!(storage::MAX_FILE_BYTES,32*1024*1024);assert_eq!(storage::MAX_SESSION_BYTES,500*1024*1024);assert_eq!(storage::MAX_FILES,500);
 assert!(storage::validate_dimensions(10000,10000).is_err());assert!(storage::validate_dimensions(2000,4000).is_ok());
}
