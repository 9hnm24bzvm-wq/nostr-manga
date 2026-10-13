# Storage health and primary-database visibility

The guarded health slice passed full regression, including a PostgreSQL-observed lock-wait test for quarantine versus a shared admission fence. This is the first maintenance foundation, not complete repair, physical garbage collection or continuous scrubbing.

## Observation and accounting

A read/hash/size verification failure on an available generation creates an idempotent, generation-fenced quarantine marker. An available object is not reclassified as an unfinished upload. Its reservation, physical charge and references remain intact. A stale observation for another generation cannot quarantine current replacement bytes.

Markers are sparse: normal available receipts do not require another health row. The blob's available state represents its durable receipt, while the marker represents a later negative observation. Do not collapse those concepts into one boolean used by failed-upload cleanup.

The marker transaction exclusively locks the blob row used to fence admission and increments a storage-health epoch only when a new marker is inserted. Publication and library-object grant transactions recheck quarantine under their generation/availability fence. A validated image schema error in otherwise correctly stored bytes is not a storage-corruption observation.

## Serving

A primary lookup gates event-body cache access, discovery and live inventory rows against quarantined references. Healthy bytes belonging only to an unhealthy publication are not advertised through its public-original route. Actual reads remain hash/size verified. A Moka hit cannot bypass the primary negative health index.

Snapshots are a separate case: immutable cached reconciliation state can outlive an object's health. Snapshot creation captures the epoch before gathering IDs and validates it at commit; each response validates it again after computing its frame. A changed epoch forces a restart rather than emitting a frame from the old healthy inventory. Normal inserts/removals retain the existing stable-snapshot semantics.

These guarantees apply to the wrapper's guarded endpoints. Directly exposing raw strfry retrieval would bypass the primary health gate. Background backend withdrawal, verified repair/unquarantine and a rotating bounded auditor remain implementation work. No claim is made that silent damage is discovered immediately.

## Failure boundaries

A failed quarantine database write prevents the read from succeeding; it does not pretend the observation was durably recorded. Database outages fail primary visibility checks closed. Operational retry/audit and durable observation recovery across failures still need dedicated tests before production deployment.

A verified repair must preserve generation/authority fencing, revalidate bytes and explicitly advance the health epoch when clearing a marker. It must not revive a locally moderated publication. Reclamation still requires all references/pins and confirmed physical cleanup; a health marker alone never authorizes deletion or refund.


Evidence: results/health-fenced-regression.log is the final full run. The shared-image HTTP fixture physically damages one image and verifies both chapters disappear from event/cache, discovery and live inventory paths while storage charges remain. The multi-process inventory fixture tests health-epoch invalidation of cached sessions. The library fixture checks quarantine waits on the actual PostgreSQL admission lock and rejects a stale generation. Initial failed test logs are retained for reproducibility; the first failure was a missing authentication header in the test's discovery request.
