//! Public, whitelisted capability registry. This is configuration, not health proof.
use crate::{
    library_manifest::Limits, library_storage::LibraryConcurrency, protocol_limits as limits,
};
use serde_json::{Value, json};

pub struct Profile<'a> {
    pub base_url: &'a str,
    pub service_key: &'a str,
    pub formats: &'a [String],
    pub max_blob_bytes: Option<u64>,
    pub concurrency: &'a LibraryConcurrency,
    pub features: Value,
    pub staging_reclamation: bool,
    pub read_policy: crate::relay::ReadPolicy,
    pub review_limits: Option<&'a crate::submission_review::ReviewLimits>,
    pub cors_origins: &'a [String],
}
impl Profile<'_> {
    pub fn document(&self) -> Value {
        let library = Limits::default();
        let read_policy = match self.read_policy {
            crate::relay::ReadPolicy::Public => "public",
            crate::relay::ReadPolicy::Authenticated => "authenticated",
            crate::relay::ReadPolicy::Admitted => "admitted",
        };
        json!({
            "namespace_version":1,"experimental":true,"api_versions":[1],
            "service_pubkey":self.service_key,"base_url":self.base_url,
            "capability_semantics":"configured_support_not_health_or_authorization",
            "features":self.features,
            "access":{"read_policy":read_policy,"publication":"admitted_publisher","replication":"approved_peer","account_writes":"admitted_account"},
            "required_features":{"series":[crate::series_metadata::COVER_FEATURE]},"schemas":{"chapter":[1],"series":[1],"library_root":[1],"library_chunk":[1]},
            "event_kinds":{"service_announcement":crate::service_announcement::KIND,"chapter":1001,"series":1002,"library":11999},
            "image_formats":self.formats,
            "endpoints":{"service_announcement":"/service/announcement","cached_service_announcement":"/service/announcements/{pubkey}","identity":"/identity","chapter_query":"/query","series_query":"/series","catalogue_entry":"/catalogue/{id}","publication":"/publications","upload":"/upload/{sha256}","staged_blob":"/blobs/{sha256}","original":"/originals/{sha256}","blossom_upload":"/upload","blossom_blob":"/{sha256}[.extension]"},
            "blob_protocol":{"profile":"server_scoped_private_staging","standard_blossom":false,"standard_routes":true,"implemented_buds":[1,2,11,12],"optional_list":false,"optional_upload_preflight":false,"staging_urls_require_authorization":true,"authorization":{"event_kind":24242,"encoding":"base64url_no_padding","server_scope":"required_lowercase_domain","hash_scope":"required_upload_delete_optional_get","max_lifetime_seconds":300},"legacy_routes":{"server_scope":"required_full_origin","encoding":["base64","base64url_no_padding"]}},
            "compatibility":{"mangadex":{"prefix":"/compat/mangadex","profile":"content_reads_and_nostr_managed_writes_v1","full_drop_in":false,"page_limit":100,"response_bytes":2097152,"aggregate_chapters":5000,"publications_immutable":true,"password_auth":false,"reader_accounts":true,"public_lists":true,"reading_progress":true,"signed_library_writes":true,"managed_group_profiles":true,"group_publication_retries":"persist_created_at_and_payload","library_pagination":"head_pinned_bucket_key","library_snapshot_conflict_status":409}},
            "submission_review":{"enabled":self.review_limits.is_some(),"limits":self.review_limits,"admission":"admitted_account","approval":"instance_operator","publisher_permission_granted":false,"intake":"/submissions"},
            "cors":{"blossom_and_nip11_origins":["*"],"application_origins":self.cors_origins,"credentials":false},
            "lifecycle":{"immutable_publications":true,"replacement_links":false,"originals_required":true,"staging_reclamation":self.staging_reclamation,"shared_blob_deletion":"refused_while_protected","publication_deletion":"instance_policy"},
            "limits":{"series_metadata":{"title_bytes":1024,"localized_entries":64,"alternative_title_maps":128,"description_bytes":65536,"categories":256,"category_bytes":512,"credits":128},
                "blob_bytes":self.max_blob_bytes,
                "discovery":{"query_bytes":limits::QUERY_BYTES,"page_items":limits::QUERY_LIMIT,"filter_bytes":limits::FILTER_BYTES,"filter_tokens":limits::FILTER_TOKENS,"filter_depth":limits::FILTER_DEPTH,"filter_fields":limits::FILTER_FIELDS,"pagination":"live_offset","timestamp_bounds":true,"explicit_order_required":true,"exact_total_optional":true},
                "library":{"wire_bytes":crate::library_wire::MAX_WIRE,"wire_objects":crate::library_wire::MAX_OBJECTS,"root_bytes":library.max_root_bytes,"chunk_bytes":library.max_chunk_bytes,"chunk_records":library.max_chunk_records,"family_buckets":library.max_family_buckets,"total_chunks":library.max_total_chunks,"records":library.max_records,"total_bytes":library.max_total_bytes,"request_slots":self.concurrency.request_slots,"processing_slots":self.concurrency.processing_slots,"commit_slots":self.concurrency.commit_slots,"view_page_items":100,"view_page_scan_bytes":16777216,"view_lookup_keys":5000,"view_lookup_scan_bytes":67108864}
            }
        })
    }
}
pub fn relay_limits(auth_required: bool) -> Value {
    json!({"max_message_length":limits::WS_MESSAGE_BYTES,"max_subscriptions":limits::RELAY_SUBSCRIPTIONS,"max_filters":limits::RELAY_FILTERS,"max_limit":limits::RELAY_LIMIT,"max_subid_length":limits::RELAY_SUBID_BYTES,"auth_required":auth_required,"restricted_writes":true})
}

#[cfg(test)]
mod public_contract_tests {
    use crate::{
        capabilities::{Profile, relay_limits},
        discovery::QueryOptions,
        library_storage::LibraryConcurrency,
        relay::ReadPolicy,
    };
    use serde_json::json;

    #[test]
    fn capability_document_reports_configured_support_without_granting_access() {
        let concurrency = LibraryConcurrency {
            request_slots: 7,
            processing_slots: 3,
            commit_slots: 2,
            ..LibraryConcurrency::default()
        };
        let formats = vec!["image/png".to_owned()];
        let origins = vec!["https://reader.example".to_owned()];
        let document = Profile {
            base_url: "https://node.example",
            service_key: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            formats: &formats,
            max_blob_bytes: Some(4096),
            concurrency: &concurrency,
            features: json!({"library":true}),
            staging_reclamation: true,
            read_policy: ReadPolicy::Admitted,
            review_limits: None,
            cors_origins: &origins,
        }
        .document();
        assert_eq!(document["base_url"], "https://node.example");
        assert_eq!(
            document["service_pubkey"],
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
        assert_eq!(
            document["capability_semantics"],
            "configured_support_not_health_or_authorization"
        );
        assert_eq!(document["image_formats"], json!(["image/png"]));
        assert_eq!(document["limits"]["blob_bytes"], 4096);
        assert_eq!(document["limits"]["library"]["request_slots"], 7);
        assert_eq!(document["limits"]["library"]["processing_slots"], 3);
        assert_eq!(document["limits"]["library"]["commit_slots"], 2);
        assert_eq!(
            document["access"],
            json!({"read_policy":"admitted", "publication":"admitted_publisher", "replication":"approved_peer", "account_writes":"admitted_account"})
        );
        assert_eq!(
            document["cors"]["application_origins"],
            json!(["https://reader.example"])
        );
        assert_eq!(document["cors"]["credentials"], false);
        assert_eq!(document["submission_review"]["enabled"], false);
        assert_eq!(
            document["submission_review"]["publisher_permission_granted"],
            false
        );
        assert_eq!(
            document["required_features"]["series"],
            json!(["series-cover-v1"])
        );
        let limit = document["limits"]["discovery"]["page_items"]
            .as_u64()
            .unwrap();
        assert_eq!(limit, 1000);
        assert!(QueryOptions::parse(Some(&format!("order=asc&limit={limit}"))).is_ok());
        assert!(QueryOptions::parse(Some(&format!("order=asc&limit={}", limit + 1))).is_err());
        let bytes = document["limits"]["discovery"]["query_bytes"]
            .as_u64()
            .unwrap() as usize;
        assert!(QueryOptions::parse(Some(&"x".repeat(bytes + 1))).is_err());
        for required in [false, true] {
            assert_eq!(relay_limits(required)["auth_required"], required);
            assert_eq!(relay_limits(required)["restricted_writes"], true);
            assert_eq!(relay_limits(required)["max_limit"], 500);
        }
    }
}
