# Historical performance plan: paused

The user prioritized complete features over further performance work. Do not run the following historical workload plan as part of current feature development. Current checks are functional regressions and recovery scenarios; production capacity remains unqualified.

# Current release workload checks

Scope: the verified managed-serving/schema35 checkpoint, using the release binary whose SHA256 is in results/serving-role-release-binary.sha256. Functional graph tickets01–17 are complete for the documented reference profile; ticket18 production capacity remains infrastructure-blocked.

1. Repeat the actual-node30,000- and500,000-completed-chapter serial profiles. Preserve local and application-paced network results separately. Twelve saves per profile is a smoke/latency sample; its p95 is effectively its maximum, not a production tail estimate.
2. Include real client verification of request-bound service acknowledgement signatures and public library-head author signatures. The old comparison driver is frozen under bench/library-node/versions/run-before-client-response-proofs.py. Content hashes and CBOR object sizes/counts remain checked during restoration. The updated verification has positive/tamper/replay tests.
3. Run128 concurrent synthetic readers (126 normal,2 extreme) at120 scheduled saves/s for360seconds, with library reclamation enabled. The duration crosses the five-minute cleanup grace. Preserve all rejected attempts and scheduled latency, not only successful request times. Generator cold state is compacted only after initial import; identical-update tests cover both library sizes.
4. Retain exact binary/driver hashes, host description, PostgreSQL fsync/synchronous-commit state, final queue drain, sampled node RSS, and accounting conservation checks. Temporary fixtures use only synthetic keys and data, loopback sockets and the workspace overlay.

The120/s rate is a rounded average baseline for300M saves in a30-day month. It is not a burst requirement, combined image/page/save workload, multi-region test, physical durability proof or100TB working-set validation. Node RSS excludes PostgreSQL, strfry, the load generator and kernel caches. No paid or public infrastructure is started.
