# 34: Implement optional reader enrollment

Status: done

## Acceptance

- Disabled by default; only personal account permission granted.
- Existing operator denials and mixed-version revocations cannot be undone.
- Admission and signup-budget counters commit atomically with tested races.
- Public policy discovery and signed metadata/library workflows verified.

## Evidence

- results/enrollment-final-full.log
- results/enrollment-upgrade-fences.log
- results/enrollment-python.log
- docs/reader-enrollment.md
- deliverables/checkpoint25-receipt.json
