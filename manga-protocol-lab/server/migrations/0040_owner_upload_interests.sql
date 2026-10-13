-- Ordinary administrator uploads need explicit temporary interests too, so a
-- different uploader cannot erase a shared staged original. Legacy unretained
-- owned data receives one conservative upgrade grace rather than guessed history.
INSERT INTO publisher_blob_grants(publisher,hash,generation,permission_version,lease_until)
 SELECT b.owner,b.hash,b.generation,p.version,GREATEST(b.staging_until,clock_timestamp()+interval '5 minutes')
 FROM blob_intents b JOIN blob_retention_credits r ON r.hash=b.hash AND r.generation=b.generation
 JOIN publisher_permissions p ON p.publisher=b.owner
 WHERE b.state='available' AND NOT r.staging_released
 ON CONFLICT(publisher,hash) DO UPDATE SET generation=EXCLUDED.generation,
 permission_version=EXCLUDED.permission_version,
 lease_until=GREATEST(publisher_blob_grants.lease_until,EXCLUDED.lease_until);
