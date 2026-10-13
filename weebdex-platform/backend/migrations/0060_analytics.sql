-- Bounded anonymous local view aggregation. No fingerprint or IP address is stored in plaintext.
CREATE TABLE platform.analytics_rate_limits (
 peer_hash BYTEA NOT NULL CHECK(octet_length(peer_hash)=32), bucket BIGINT NOT NULL,
 count INTEGER NOT NULL CHECK(count>0), PRIMARY KEY(peer_hash,bucket)
);
CREATE INDEX analytics_rate_expiry ON platform.analytics_rate_limits(bucket);
CREATE FUNCTION platform.analytics_user_stats(value TEXT) RETURNS JSONB LANGUAGE SQL STABLE AS $$
 SELECT jsonb_build_object('chapters',(SELECT count(*) FROM platform.catalog_chapters c JOIN platform.catalog_manga m ON m.id=c.manga_id WHERE c.uploader_id=value AND c.deleted_at IS NULL AND m.deleted_at IS NULL AND m.state='published'),
 'messages',(SELECT count(*) FROM platform.community_replies r WHERE r.author_id=value AND r.state IN ('active','locked','flagged') AND platform.community_thread_visible(r.thread_id)),
 'points',COALESCE((SELECT sum(CASE WHEN v.vote=1 THEN 1 ELSE -1 END) FROM platform.community_reply_votes v JOIN platform.community_replies r ON r.id=v.reply_id WHERE r.author_id=value AND r.state IN ('active','locked','flagged') AND platform.community_thread_visible(r.thread_id)),0))
 FROM platform.users u WHERE u.id=value AND NOT u.disabled
$$;
