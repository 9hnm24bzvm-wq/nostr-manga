# 36: Preserve retryable admission failures

Status: done (final71-group regression passed)

## Acceptance

- True missing/revoked authority stays403.
- Account, group and peer permission database errors return503.
- Metadata transaction failures are not invalid-client400 errors.
- Fresh authorization retry succeeds after service-store recovery.

## Evidence

- results/admission-error-focused.log
- results/admission-errors.json

- results/backend-mvp-final-full.log
