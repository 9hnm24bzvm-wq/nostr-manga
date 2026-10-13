-- Avoid scanning historical sealed grants in hot lane selection/refill paths.
CREATE INDEX capacity_grants_active_holder ON capacity_grants(budget_key,holder,available,id) WHERE state='active';
CREATE INDEX capacity_grants_reclaimable_holder ON capacity_grants(budget_key,holder,id) WHERE state='active' OR available>0;
