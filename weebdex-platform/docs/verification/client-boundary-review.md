# Client boundary review checklist

New anonymous and signed readers must bound bytes, fragment/object growth, deadlines and retained page data. Track fetch/read/cancel settlement; a finite cleanup timeout must retain the active ownership fence until work actually settles. Never fire-and-forget cancellation or cancel healthy EOF. Clear stale target data synchronously before queued navigation work.

Use function declarations for local `never`-returning assertion/failure helpers so TypeScript control-flow narrowing is preserved. Compile the actual composed consumer, not only a transpiled fixture. Project retained HTTP response boundaries explicitly and validate absent/contradictory bodies; do not hide incompatible optional fields with casts.

Keep old-source controls and prior failed receipts. Candidate finite tests, composed strict checks, real HTTP/concurrency, native browser behavior and capacity acceptance are separate evidence.
