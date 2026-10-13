# Historical candidate plan

The verified bounded result is now in query-cache-results.md. The candidate notes below precede that result.

# Fixed-query cache candidate

Unreleased follow-through after source checkpoint21. The 64-credit-lane test also failed:71,995 of72,000 writes accepted, five429 work-queue timeouts and752.71seconds against600 scheduled. It did not establish an improvement and does not change defaults.

The next candidate uses deadpool-postgres's existing per-connection prepared-statement cache for102 fixed SQL call sites in capacity, storage accounting, library/storage, reclamation, blob admission, backend acknowledgement and library history. It adds no dependency, schema change, result cache or cached authorization decision. The set of SQL strings is compile-time bounded; request values remain bound parameters.

The mechanical transformation preserves SQL text, arguments, transaction boundaries, lock order, generation checks and receipt-before-refund sequencing. Reversing only the wrappers restores the original source modulo whitespace; results/query-cache-transform.json and experiments/query-cache-candidate.patch preserve that check. It does not remove any database round trip or weaken a fence; it aims to avoid repeatedly preparing/planning identical statements and reduce time inside lock-holding transactions.

Prepared statements can use generic plans. The sparse500k-row retention fixture now executes the cached query repeatedly and explicitly verifies both force_custom_plan and force_generic_plan retain bounded indexed work. This focused fixture, Clippy and PostgreSQL/process restart checks pass. The complete regression and optimized build must pass before performance testing; neither a capacity fix nor a new release is claimed yet.

The next comparison returns to16 credit lanes, keeping the previously measured32 database connections,16 processing slots,8 commits,16 storage slots and8 cleanup workers. Preserve scheduled latency, rejections, cleanup debt, final accounting and process diagnostics. Do not infer success from faster individual queries alone.
