-- A budget grants durable byte credits in batches. Object operations touch their
-- grant rows, not a single instance-wide counter on every upload.
CREATE TABLE capacity_budgets (
    key TEXT COLLATE "C" PRIMARY KEY,
    limit_bytes BIGINT NOT NULL CHECK(limit_bytes>0),
    allocated_bytes BIGINT NOT NULL DEFAULT 0 CHECK(allocated_bytes>=0 AND allocated_bytes<=limit_bytes)
);
CREATE TABLE capacity_grants (
    id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
    budget_key TEXT NOT NULL REFERENCES capacity_budgets(key),
    holder BYTEA NOT NULL CHECK(octet_length(holder)=32),
    capacity BIGINT NOT NULL CHECK(capacity>=0),
    available BIGINT NOT NULL CHECK(available>=0),
    reserved BIGINT NOT NULL DEFAULT 0 CHECK(reserved>=0),
    used BIGINT NOT NULL DEFAULT 0 CHECK(used>=0),
    state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active','sealed')),
    CHECK(available+reserved+used=capacity),
    UNIQUE(id,budget_key)
);
CREATE FUNCTION account_capacity_grant() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        UPDATE capacity_budgets SET allocated_bytes=allocated_bytes+NEW.capacity WHERE key=NEW.budget_key;
        RETURN NEW;
    ELSIF TG_OP='DELETE' THEN
        UPDATE capacity_budgets SET allocated_bytes=allocated_bytes-OLD.capacity WHERE key=OLD.budget_key;
        RETURN OLD;
    ELSIF NEW.capacity<>OLD.capacity THEN
        UPDATE capacity_budgets SET allocated_bytes=allocated_bytes+(NEW.capacity-OLD.capacity) WHERE key=NEW.budget_key;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER capacity_grant_insert_delete AFTER INSERT OR DELETE ON capacity_grants FOR EACH ROW EXECUTE FUNCTION account_capacity_grant();
CREATE TRIGGER capacity_grant_resize AFTER UPDATE OF capacity ON capacity_grants FOR EACH ROW EXECUTE FUNCTION account_capacity_grant();
CREATE FUNCTION immutable_grant_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.id<>OLD.id OR NEW.budget_key<>OLD.budget_key OR NEW.holder<>OLD.holder THEN
        RAISE EXCEPTION 'grant identity is immutable';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER capacity_grant_identity BEFORE UPDATE ON capacity_grants FOR EACH ROW EXECUTE FUNCTION immutable_grant_identity();
CREATE TABLE capacity_reservations (
    id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
    requested BIGINT NOT NULL CHECK(requested>0),
    actual BIGINT CHECK(actual IS NULL OR actual>=0 AND actual<=requested),
    state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','settled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE capacity_charges (
    reservation BYTEA NOT NULL REFERENCES capacity_reservations(id),
    grant_id BYTEA NOT NULL,
    budget_key TEXT NOT NULL,
    charged BIGINT NOT NULL DEFAULT 0 CHECK(charged>=0),
    refunded BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY(reservation,grant_id),
    UNIQUE(reservation,budget_key),
    FOREIGN KEY(grant_id,budget_key) REFERENCES capacity_grants(id,budget_key)
);
CREATE INDEX capacity_grants_budget ON capacity_grants(budget_key,id);
CREATE INDEX capacity_charges_grant ON capacity_charges(grant_id,reservation);
