-- Reports and audit decisions are private local instance state, never relay events.
CREATE TABLE platform.report_reasons (
 id TEXT PRIMARY KEY, category TEXT NOT NULL CHECK(category IN ('user','manga','author','chapter','scanlation_group','thread','reply')),
 reason TEXT NOT NULL, details_required BOOLEAN NOT NULL DEFAULT FALSE, enabled BOOLEAN NOT NULL DEFAULT TRUE,
 UNIQUE(id,category)
);
INSERT INTO platform.report_reasons(id,category,reason,details_required)
 SELECT category || '_spam',category,'Spam or abusive content',TRUE FROM unnest(ARRAY['user','manga','author','chapter','scanlation_group','thread','reply']) category;
INSERT INTO platform.report_reasons(id,category,reason,details_required)
 SELECT category || '_incorrect',category,'Incorrect information',TRUE FROM unnest(ARRAY['user','manga','author','chapter','scanlation_group','thread','reply']) category;
INSERT INTO platform.report_reasons(id,category,reason,details_required)
 SELECT category || '_other',category,'Other issue',TRUE FROM unnest(ARRAY['user','manga','author','chapter','scanlation_group','thread','reply']) category;
INSERT INTO platform.report_reasons(id,category,reason,details_required) VALUES('scanlation_group_claim','scanlation_group','Group claim request',TRUE);
CREATE TABLE platform.reports (
 id TEXT PRIMARY KEY, submitter_id TEXT NOT NULL REFERENCES platform.users(id), entity_type TEXT NOT NULL,
 entity_id TEXT NOT NULL, reason_id TEXT NOT NULL, target_version INTEGER, body TEXT NOT NULL DEFAULT '' CHECK(octet_length(body)<=20000),
 status TEXT NOT NULL DEFAULT 'waiting' CHECK(status IN ('waiting','accepted','refused','autoresolved')),
 notes TEXT CHECK(octet_length(notes)<=20000), reviewed_by TEXT REFERENCES platform.users(id),
 version INTEGER NOT NULL DEFAULT 1 CHECK(version>0), created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(), deleted_at TIMESTAMPTZ,
 FOREIGN KEY(reason_id,entity_type) REFERENCES platform.report_reasons(id,category)
);
CREATE INDEX reports_private_feed ON platform.reports(submitter_id,created_at DESC,id) WHERE deleted_at IS NULL;
CREATE INDEX reports_review_queue ON platform.reports(status,created_at,id) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX reports_waiting_unique ON platform.reports(submitter_id,entity_type,entity_id,reason_id) WHERE status='waiting' AND deleted_at IS NULL;
CREATE TABLE platform.report_audit (
 id BIGSERIAL PRIMARY KEY, report_id TEXT NOT NULL REFERENCES platform.reports(id), actor_id TEXT NOT NULL REFERENCES platform.users(id),
 operation TEXT NOT NULL CHECK(operation IN ('create','accept','reject','delete')), old_status TEXT, new_status TEXT,
 old_version INTEGER, new_version INTEGER NOT NULL, notes TEXT, effect JSONB NOT NULL DEFAULT '{}'::jsonb, created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION platform.report_entity_exists(kind TEXT,value TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT CASE kind
 WHEN 'user' THEN EXISTS(SELECT 1 FROM platform.users u WHERE u.id=value AND NOT u.disabled)
 WHEN 'manga' THEN EXISTS(SELECT 1 FROM platform.catalog_manga m WHERE m.id=value AND m.state='published' AND m.deleted_at IS NULL)
 WHEN 'author' THEN EXISTS(SELECT 1 FROM platform.catalog_authors a WHERE a.id=value AND a.deleted_at IS NULL)
 WHEN 'chapter' THEN EXISTS(SELECT 1 FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.id=value AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published')
 WHEN 'scanlation_group' THEN EXISTS(SELECT 1 FROM platform.catalog_groups g WHERE g.id=value AND g.deleted_at IS NULL)
 WHEN 'thread' THEN platform.community_thread_visible(value)
 WHEN 'reply' THEN EXISTS(SELECT 1 FROM platform.community_replies r WHERE r.id::TEXT=value AND r.state='active' AND platform.community_thread_visible(r.thread_id))
 ELSE FALSE END
$$;
CREATE FUNCTION platform.report_json(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_strip_nulls(jsonb_build_object('id',r.id,'created_at',platform.catalog_timestamp(r.created_at),'updated_at',platform.catalog_timestamp(r.updated_at),'status',r.status,'body',r.body,'notes',r.notes,'version',r.version,
 'relationships',jsonb_strip_nulls(jsonb_build_object('submitter',platform.community_user_json(r.submitter_id),
 'reason',jsonb_build_object('id',rr.id,'category',rr.category,'reason',rr.reason,'details_required',rr.details_required),
 'user',CASE WHEN r.entity_type='user' THEN platform.community_user_json(r.entity_id) END,
 'manga',CASE WHEN r.entity_type='manga' AND platform.report_entity_exists(r.entity_type,r.entity_id) THEN platform.catalog_manga_json(r.entity_id) END,
 'author',CASE WHEN r.entity_type='author' THEN platform.catalog_author_json(r.entity_id) END,
 'chapter',CASE WHEN r.entity_type='chapter' AND platform.report_entity_exists(r.entity_type,r.entity_id) THEN platform.catalog_chapter_json(r.entity_id) END,
 'group',CASE WHEN r.entity_type='scanlation_group' THEN platform.catalog_group_json(r.entity_id) END,
 'thread',CASE WHEN r.entity_type='thread' THEN platform.community_thread_json(r.entity_id) END,
 'reply',CASE WHEN r.entity_type='reply' THEN (SELECT platform.community_reply_json(x.id,NULL) || jsonb_build_object('relationships',(platform.community_reply_json(x.id,NULL)->'relationships') || jsonb_build_object('thread',platform.catalog_thread_json(t.entity_type,t.entity_id))) FROM platform.community_replies x JOIN platform.catalog_threads t ON t.id=x.thread_id WHERE x.id::TEXT=r.entity_id) END))))
 FROM platform.reports r JOIN platform.report_reasons rr ON rr.id=r.reason_id WHERE r.id=value AND r.deleted_at IS NULL
$$;
