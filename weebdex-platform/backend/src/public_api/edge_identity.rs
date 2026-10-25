//! Explicit reconstructed v2 composite identity. Namespace and kind are included
//! by public_native_uuid; direction and UTF-8 byte lengths belong to this key.
//! Existing persisted aliases always win and are never rewritten.
pub fn manga_edge_key(source: &str, target: &str) -> String {
    format!(
        "edge-v2:{}:{}{}:{}",
        source.len(),
        source,
        target.len(),
        target
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composite_identity_is_unambiguous_directional_and_versioned() {
        assert_ne!(manga_edge_key("a", "bc"), manga_edge_key("ab", "c"));
        assert_ne!(manga_edge_key("a:b", "c"), manga_edge_key("a", "b:c"));
        assert_ne!(
            manga_edge_key("source", "target"),
            manga_edge_key("target", "source")
        );
        assert_eq!(manga_edge_key("é", "x"), "edge-v2:2:é1:x");
    }
}
