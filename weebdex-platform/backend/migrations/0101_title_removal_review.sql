-- Dedicated signed title removal is review-first for ordinary owners. This
-- operation-only reason is disabled in generic creation/reason discovery.
INSERT INTO platform.report_reasons(id,category,reason,details_required,enabled)
VALUES('manga_removal_request','manga','Current owner requests instance title removal',FALSE,FALSE);
