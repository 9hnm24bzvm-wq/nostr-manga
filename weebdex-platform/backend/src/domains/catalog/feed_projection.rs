//! One-statement, request-local reuse of expensive nested catalogue DTOs.
//! `selected` is already paginated. Materialization makes projection work
//! proportional to distinct selected parents/groups, not the chapter count.

pub(super) fn chapter_page_projection(order: &str, limit: i64, page: i64, mapped: bool) -> String {
    // `order` is the validated ASC/DESC vocabulary from QueryParams::order.
    let map = if mapped {
        ",'map',jsonb_build_object('manga',COALESCE((SELECT jsonb_object_agg(manga_id,value) FROM parent_dtos),'{}'::jsonb))"
    } else {
        ""
    };
    format!(
        ",parent_dtos AS MATERIALIZED(SELECT manga_id,platform.catalog_manga_json(manga_id) AS value FROM(SELECT DISTINCT manga_id FROM selected)ids),group_dtos AS MATERIALIZED(SELECT g.id,platform.catalog_group_json(g.id) AS value FROM platform.catalog_groups g JOIN(SELECT DISTINCT cg.group_id FROM platform.catalog_chapter_groups cg JOIN selected ON selected.id=cg.chapter_id)ids ON ids.group_id=g.id WHERE g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)) SELECT jsonb_build_object('total',(SELECT count(*) FROM matched),'limit',{limit},'page',{page},'data',COALESCE((SELECT jsonb_agg(platform.catalog_chapter_json(selected.id,parent_dtos.value,COALESCE((SELECT jsonb_agg(group_dtos.value ORDER BY cg.group_id) FROM platform.catalog_chapter_groups cg JOIN group_dtos ON group_dtos.id=cg.group_id WHERE cg.chapter_id=selected.id),'[]'::jsonb)) || jsonb_build_object('_sort_value',sort_value,'_sort_text',sort_text) ORDER BY sort_value {order} NULLS LAST,sort_text {order},selected.id) FROM selected JOIN parent_dtos ON parent_dtos.manga_id=selected.manga_id),'[]'::jsonb){map})"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_migrations_keep_the_exact_pending_pr28_prerequisite() {
        use sha2::{Digest, Sha256};
        let migrations = crate::domains::MIGRATIONS;
        assert!(
            migrations
                .windows(2)
                .all(|pair| pair[0].version < pair[1].version)
        );
        assert_eq!(
            migrations[migrations.len() - 3..]
                .iter()
                .map(|m| m.version)
                .collect::<Vec<_>>(),
            [85, 86, 87]
        );
        let prerequisite = migrations.iter().find(|m| m.version == 86).unwrap();
        assert_eq!(prerequisite.name, "0086_visibility_policy_preparation");
        assert_eq!(
            hex::encode(Sha256::digest(prerequisite.sql.as_bytes())),
            "04d617d8836d47d7ab572c03ac7dc2e3a5b362415bffe34f99d94d3ed07b87b6"
        );
    }

    #[test]
    fn projection_is_selected_only_materialized_and_reused_by_map() {
        for order in ["ASC", "DESC"] {
            for (limit, page) in [(0, 1), (1, 6), (100, 1), (100, 6)] {
                for mapped in [false, true] {
                    let sql = chapter_page_projection(order, limit, page, mapped);
                    assert!(sql.starts_with(",parent_dtos AS MATERIALIZED("));
                    assert!(sql.contains("group_dtos AS MATERIALIZED("));
                    assert!(sql.contains("FROM(SELECT DISTINCT manga_id FROM selected)ids"));
                    assert!(sql.contains("SELECT DISTINCT cg.group_id FROM platform.catalog_chapter_groups cg JOIN selected ON selected.id=cg.chapter_id"));
                    assert_eq!(sql.matches("platform.catalog_manga_json(").count(), 1);
                    assert_eq!(sql.matches("platform.catalog_group_json(").count(), 1);
                    assert_eq!(
                        sql.matches("platform.catalog_chapter_json(selected.id,parent_dtos.value,")
                            .count(),
                        1
                    );
                    assert!(sql.contains(
                        "g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)"
                    ));
                    assert!(sql.contains("jsonb_agg(group_dtos.value ORDER BY cg.group_id)"));
                    assert!(sql.contains(&format!(
                        "ORDER BY sort_value {order} NULLS LAST,sort_text {order},selected.id"
                    )));
                    assert!(sql.contains(&format!("'limit',{limit},'page',{page}")));
                    assert!(sql.contains("'total',(SELECT count(*) FROM matched)"));
                    assert_eq!(sql.contains("'map',jsonb_build_object"), mapped);
                    if mapped {
                        assert!(sql.contains("jsonb_object_agg(manga_id,value) FROM parent_dtos"));
                    }
                }
            }
        }
    }

    #[test]
    fn overload_preserves_every_canonical_field_and_visibility_predicate() {
        let canonical = include_str!("../../../migrations/0082_catalog_projection_visibility.sql")
            .split("CREATE OR REPLACE FUNCTION platform.catalog_chapter_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$\n")
            .nth(1).unwrap().split("\n$$;").next().unwrap();
        let group_projection = canonical
            .split("'groups',")
            .nth(1)
            .unwrap()
            .split(",'thread'")
            .next()
            .unwrap();
        assert_eq!(
            canonical
                .matches("platform.catalog_manga_json(c.manga_id)")
                .count(),
            1
        );
        assert_eq!(canonical.matches(group_projection).count(), 1);
        let expected = canonical
            .replace("platform.catalog_manga_json(c.manga_id)", "parent_json")
            .replace(group_projection, "groups_json");
        let overload =
            include_str!("../../../migrations/0087_catalog_chapter_relationship_projection.sql")
                .split("AS $$\n")
                .nth(1)
                .unwrap()
                .split("\n$$;")
                .next()
                .unwrap();
        assert_eq!(overload, expected);
    }
}
