-- Public signed chapter removal is review-first. Reports remain private instance
-- moderation and never grant original protocol deletion approval.
INSERT INTO platform.report_reasons(id,category,reason,details_required,enabled)
VALUES('chapter_removal_request','chapter','Authorized uploader or group leader requests instance removal',FALSE,FALSE);

-- Dedicated DELETE creates this operation-only reason. Disabled keeps it out
-- of generic reason discovery/POST report, which has no removal preflight.
