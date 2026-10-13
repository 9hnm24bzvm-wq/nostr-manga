ALTER TABLE account_permissions ADD COLUMN enrollment_blocked BOOLEAN NOT NULL DEFAULT FALSE;
-- Existing denials may predate an explicit enrollment distinction. Preserve them
-- conservatively; only an operator can deliberately re-admit those keys.
UPDATE account_permissions SET enrollment_blocked=TRUE WHERE NOT allowed;
CREATE TABLE reader_enrollment_budget (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
    window_started TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    window_created BIGINT NOT NULL DEFAULT 0 CHECK(window_created>=0),
    total_created BIGINT NOT NULL DEFAULT 0 CHECK(total_created>=0)
);
INSERT INTO reader_enrollment_budget(singleton) VALUES(TRUE);
-- Safe mixed-version upgrades: an older binary omits the new column on inserts
-- and updates only allowed/version on revocation. Neither may reopen signup.
ALTER TABLE account_permissions ALTER COLUMN enrollment_blocked SET DEFAULT TRUE;
CREATE FUNCTION fence_account_enrollment_denial() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT NEW.allowed THEN
        NEW.enrollment_blocked := TRUE;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER account_enrollment_denial BEFORE UPDATE OF allowed ON account_permissions
FOR EACH ROW EXECUTE FUNCTION fence_account_enrollment_denial();
