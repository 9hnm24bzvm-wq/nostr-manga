-- Public-only title projection. Keep the canonical one-argument native function
-- unchanged. Do not construct its unbounded available_groups JSON only to discard
-- it: group choices are now read through an independent bounded keyset endpoint.
-- Remaining projection fields match migration0082, including source visibility.
CREATE FUNCTION platform.catalog_public_manga_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT platform.catalog_manga_base(m.id) || jsonb_build_object('_public_title_groups',true,'relationships',jsonb_strip_nulls(jsonb_build_object(
 'submitter',jsonb_build_object('id',m.owner_id,'name',m.owner_name,'version',1),
 'cover',(SELECT platform.catalog_cover_json(c.id) FROM platform.catalog_covers c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) ORDER BY c.is_main DESC,c.created_at DESC,c.id LIMIT 1),
 'tags',platform.catalog_manga_tags_json(m.id),
 'authors',COALESCE((SELECT jsonb_agg(platform.catalog_author_json(a.id) ORDER BY a.name,a.id) FROM platform.catalog_manga_creators mc JOIN platform.catalog_authors a ON a.id=mc.author_id WHERE mc.manga_id=m.id AND mc.role='author' AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)),'[]'::jsonb),
 'artists',COALESCE((SELECT jsonb_agg(platform.catalog_author_json(a.id) ORDER BY a.name,a.id) FROM platform.catalog_manga_creators mc JOIN platform.catalog_authors a ON a.id=mc.author_id WHERE mc.manga_id=m.id AND mc.role='artist' AND a.deleted_at IS NULL AND platform.protocol_source_is_visible(a.protocol_event_id)),'[]'::jsonb),
 'links',m.metadata->'links','relations',platform.catalog_relations_json(m.id),'thread',platform.catalog_thread_json('manga',m.id),
 'available_languages',COALESCE((SELECT jsonb_agg(x.language ORDER BY x.language) FROM (SELECT DISTINCT c.language FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id) AND NOT c.is_unavailable)x),'[]'::jsonb),
 'stats',jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapters c WHERE c.manga_id=m.id AND c.deleted_at IS NULL AND platform.protocol_source_is_visible(c.protocol_event_id)),'views',COALESCE(s.views,0),'follows',COALESCE(s.follows,0))))) FROM platform.catalog_manga m LEFT JOIN platform.catalog_manga_counters s ON s.manga_id=m.id WHERE m.id=value AND m.deleted_at IS NULL AND m.protocol_source_visible AND platform.protocol_source_is_visible(m.protocol_series_id)
$$;

-- Public chapter/count/feed reads must not reintroduce the legacy title-wide
-- group aggregation through their nested parent. Delegate chapter fields, null
-- stripping and visibility to the latest three-argument native function (0088).
CREATE FUNCTION platform.catalog_public_chapter_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT platform.catalog_chapter_json(c.id,platform.catalog_public_manga_json(c.manga_id),
 COALESCE((SELECT jsonb_agg(platform.catalog_group_json(cg.group_id) ORDER BY cg.group_id) FROM platform.catalog_chapter_groups cg JOIN platform.catalog_groups g ON g.id=cg.group_id WHERE cg.chapter_id=c.id AND g.deleted_at IS NULL AND platform.protocol_group_is_visible(g.id)),'[]'::jsonb))
 FROM platform.catalog_chapters c WHERE c.id=value
$$;
