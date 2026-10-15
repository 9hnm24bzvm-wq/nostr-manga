# Instance-local manga edges v1

GET `/api/v1/manga/{id}/relation` returns the actual directed native relation set,
with immutable `manga_relation` UUIDs distinct from both endpoint manga UUIDs.
GET `/api/v1/manga/{id}/recommendation` returns up to20 actual recommendation rows,
with independent `manga_recommendation` UUIDs and persisted numeric scores.

Both endpoints require publicly visible source and target manga: published,
not deleted, cached source-visible and original-live-visible. Wrong-kind UUIDs
and hidden/unknown sources are indistinguishable404. GET never creates aliases.
Stored aliases use migration90's explicitly versioned edge-v2 length-framed
source/target codec, catalogue namespace and resource kind. Deleting/re-adding
the same ordered edge retains its canonical alias; reversing an edge or changing
its kind has a different identity. Existing stored aliases are never recomputed.

The source manga is selected by the route. Each edge's standard manga relationship
identifies its target; `includes[]=manga` expands actual target attributes. Relation
attributes contain the actual relation enum. Native edges have no independent
revision column, so the optional version is omitted rather than copied from an
unrelated manga revision. Relation views are unpaginated, with an explicit1000-edge
budget; larger sets fail instead of silently truncating.

Recommendations accept only `includes[]=manga`, `order[score]=asc|desc` (default
desc) and repeated `contentRating[]`. Default ratings are safe/suggestive/erotica;
all four standard ratings are supported. Counts and the top20 selection share one
SQL visibility snapshot; ties use native target identity deterministically. Scores
are the existing instance-local editorial values, not a claim to reproduce a
remote recommendation algorithm. The response explicitly records this provenance.

The handlers reject unsupported query fields instead of ignoring them. Pending
POST/DELETE relation transport remains explicit501. Source implementation and
fresh evidence must be checked before operation coverage is updated.
