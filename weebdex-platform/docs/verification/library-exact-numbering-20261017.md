# Exact public chapter/volume library ordering

An independent two-row reproduction over committed account/library source
`0bbb4c00431c5b71f1d18151d4db6b13452076c8` found that valid numbering
`9.99999999999999999999` and `10` both became JavaScript Number10. The public
global comparator then used their actual text as a tie-break and reversed numeric
order, for chapter and volume in both ascending and descending directions. The
backend's primary numeric order uses PostgreSQL NUMERIC, without that rounding.

The public hydration helper now keeps numeric strings and sets the existing
`_sort_decimal` metadata marker used by exact Manga NUMERIC keys. The unchanged
shared comparator already calls `compareDecimal`. No native catalogue module or
native transport path is changed. Existing actual-text secondary ties and stable
public UUID ties are retained; specials/nulls remain after numeric values in both
directions. This does not claim the public tie convention equals an internal
native-ID SQL tie-break.

All25 focused finite tests pass:19 prior cases plus6 meaningful new cases covering
both field/direction combinations and equal-number/text/UUID/special/null ties.
The old1.5-key assertion now explicitly expects exact string1.5 with the decimal
marker rather than a rounded Number; its actual text assertion is preserved.

`test-evidence/frontend/library-exact-numbering-20261017/` contains the independent
before reproduction, original compressed hydration/comparator snapshots, the
original-workspace repro script, final raw test output and exact source hashes.
These use an explicit transport stand-in and actual production helper/comparator
modules. No live HTTP/browser or combined strict/build proof is claimed. The
library owner's active worktree was not edited; this fix is isolated on its pinned
committed source and still needs the parent's authorized integration/qualification.
