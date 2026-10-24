//! Ordering contracts for bounded reader groups and their editions.
//!
//! Native order is the order of `read::chapter_aggregate`, followed by the
//! serde_json default BTreeMap / JavaScript Object.entries edition order.
//! Public order is `public-api/reader-order.ts`; edition insertion order comes
//! from the public `/chapter` collection's default published_at DESC, id ASC.
//! These are deliberately different profiles, not a new shared label order.
//!
//! SQL aliases are trusted identifiers supplied by callers, never request data.
//! Catalogue labels are at most 32 characters, so PostgreSQL NUMERIC preserves
//! every accepted digit without floating-point conversion or precision loss.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Profile {
    Native,
    Public,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Asc,
    Desc,
}

impl Direction {
    fn reversed(self, reverse: bool) -> Self {
        match (self, reverse) {
            (Self::Asc, true) => Self::Desc,
            (Self::Desc, true) => Self::Asc,
            _ => self,
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }

    fn after(self) -> &'static str {
        match self {
            Self::Asc => ">",
            Self::Desc => "<",
        }
    }
}

#[derive(Debug)]
struct Term {
    expression: String,
    direction: Direction,
}

impl Term {
    fn new(expression: impl Into<String>, direction: Direction) -> Self {
        Self {
            expression: expression.into(),
            direction,
        }
    }
}

fn numeric_label(profile: Profile, column: &str) -> String {
    match profile {
        Profile::Native => format!(
            "CASE WHEN {column} ~ '^[0-9]' THEN substring({column} FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END"
        ),
        // Absolute anchors make the contract a complete ASCII decimal, exactly
        // as reader-order.ts's non-multiline JavaScript regular expression.
        Profile::Public => format!(
            r"CASE WHEN {column} ~ E'\\A[0-9]+(?:[.][0-9]+)?\\Z' THEN {column}::numeric END"
        ),
    }
}

/// JavaScript compares strings as UTF-16 code units, not Unicode code points.
/// PostgreSQL's UTF-8 `ascii` gives a scalar value; expand supplementary scalars
/// into ordered high/low surrogates before comparing the integer arrays.
/// ARRAY(SELECT ...) is non-null, including for empty and null labels. The
/// independent null rank below keeps those two group identities distinct.
fn utf16_key(column: &str) -> String {
    format!(
        "ARRAY(SELECT __reader_units.unit FROM generate_series(1,char_length(COALESCE({column},''::text))) AS __reader_chars(position) CROSS JOIN LATERAL (SELECT ascii(substr({column},__reader_chars.position,1)) AS scalar) AS __reader_codepoint CROSS JOIN LATERAL unnest(CASE WHEN __reader_codepoint.scalar>65535 THEN ARRAY[55296+((__reader_codepoint.scalar-65536)/1024),56320+((__reader_codepoint.scalar-65536)%1024)] ELSE ARRAY[__reader_codepoint.scalar] END) WITH ORDINALITY AS __reader_units(unit,part) ORDER BY __reader_chars.position,__reader_units.part)"
    )
}

fn label_terms(profile: Profile, column: &str) -> Vec<Term> {
    let number = numeric_label(profile, column);
    let (null_rank, raw) = match profile {
        // Native raw DESC has implicit NULLS FIRST. Do not add a C collation:
        // chapter_aggregate intentionally uses the database's label collation.
        Profile::Native => (
            format!("({column} IS NOT NULL)"),
            format!("COALESCE({column},''::text)"),
        ),
        Profile::Public => (format!("({column} IS NULL)"), utf16_key(column)),
    };
    vec![
        Term::new(format!("(({number}) IS NULL)"), Direction::Asc),
        Term::new(format!("COALESCE(({number}),0::numeric)"), Direction::Desc),
        Term::new(null_rank, Direction::Asc),
        Term::new(raw, Direction::Desc),
    ]
}

fn group_terms(profile: Profile, alias: &str) -> Vec<Term> {
    let mut terms = label_terms(profile, &format!("{alias}.volume"));
    terms.extend(label_terms(profile, &format!("{alias}.chapter")));
    terms
}

/// Canonical JavaScript array-index property names are 0 through 2^32 - 2.
/// Leading zeros, signs, fractions and 2^32 - 1 are ordinary string keys.
/// The nested CASE ensures that the cast is only evaluated for <= 10 digits.
fn native_index(column: &str) -> String {
    format!(
        "CASE WHEN {column} ~ '^(0|[1-9][0-9]{{0,9}})$' THEN CASE WHEN {column}::bigint<=4294967294 THEN {column}::bigint END END"
    )
}

fn edition_terms(profile: Profile, alias: &str) -> Vec<Term> {
    match profile {
        Profile::Native => {
            let index = native_index(&format!("{alias}.id"));
            vec![
                Term::new(format!("(({index}) IS NULL)"), Direction::Asc),
                Term::new(format!("COALESCE(({index}),0::bigint)"), Direction::Asc),
                // serde_json has no preserve_order feature: ordinary keys are
                // emitted in Rust String/BTreeMap byte order before JS parsing.
                Term::new(format!("{alias}.id COLLATE \"C\""), Direction::Asc),
            ]
        }
        // These schema columns are NOT NULL. Preserve native-ID database
        // collation from public_api::catalogue::order_clause, not public UUID
        // order, JS lexical order, or the native serialized-object tie break.
        Profile::Public => vec![
            Term::new(format!("{alias}.published_at"), Direction::Desc),
            Term::new(format!("{alias}.id"), Direction::Asc),
        ],
    }
}

fn order(terms: &[Term], reverse: bool) -> String {
    terms
        .iter()
        .map(|term| {
            format!(
                "{} {}",
                term.expression,
                term.direction.reversed(reverse).sql()
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Lexicographic OR, rather than a mixed-direction row comparison. Every
/// compared expression is non-null; null-safe equality also makes each shared
/// prefix explicit. Reversal flips every component, including null ranks.
fn after(left: &[Term], right: &[Term], reverse: bool) -> String {
    assert_eq!(left.len(), right.len());
    let mut equal_prefix = Vec::with_capacity(left.len());
    let mut branches = Vec::with_capacity(left.len());
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left.direction, right.direction);
        let comparison = format!(
            "({}) {} ({})",
            left.expression,
            left.direction.reversed(reverse).after(),
            right.expression
        );
        let mut branch = equal_prefix.clone();
        branch.push(comparison);
        branches.push(format!("({})", branch.join(" AND ")));
        equal_prefix.push(format!(
            "({}) IS NOT DISTINCT FROM ({})",
            left.expression, right.expression
        ));
    }
    format!("({})", branches.join(" OR "))
}

/// ORDER BY terms for distinct (volume, chapter) groups, newest first by
/// default. `reverse` traverses the exact opposite order, including nulls.
/// The returned fragment omits the ORDER BY keyword.
pub(super) fn group_order(profile: Profile, alias: &str, reverse: bool) -> String {
    order(&group_terms(profile, alias), reverse)
}

/// Predicate that `left_alias` follows `right_alias` in group_order's traversal
/// direction. Both aliases must expose the original nullable volume/chapter.
/// Group identity itself remains (volume, chapter) IS NOT DISTINCT FROM, never
/// the parsed number: "02", "2", "2.0", null and "" are distinct groups.
pub(super) fn group_after(
    profile: Profile,
    left_alias: &str,
    right_alias: &str,
    reverse: bool,
) -> String {
    after(
        &group_terms(profile, left_alias),
        &group_terms(profile, right_alias),
        reverse,
    )
}

/// Stable within-group edition tie order. A caller's preferred-group score, if
/// any, precedes these terms; this does not sort on publication date for native.
pub(super) fn edition_order(profile: Profile, alias: &str) -> String {
    order(&edition_terms(profile, alias), false)
}

/// Predicate that an edition follows another within a group. Native callers
/// expose id; public callers expose id and non-null published_at timestamps.
pub(super) fn edition_after(profile: Profile, left_alias: &str, right_alias: &str) -> String {
    after(
        &edition_terms(profile, left_alias),
        &edition_terms(profile, right_alias),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn native_group_sql_keeps_prefix_numeric_raw_case_and_null_order() {
        let volume = "CASE WHEN c.volume ~ '^[0-9]' THEN substring(c.volume FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END";
        let chapter = "CASE WHEN c.chapter ~ '^[0-9]' THEN substring(c.chapter FROM '^[0-9]+(?:[.][0-9]+)?')::numeric END";
        assert_eq!(
            group_order(Profile::Native, "c", false),
            format!(
                "(({volume}) IS NULL) ASC,COALESCE(({volume}),0::numeric) DESC,(c.volume IS NOT NULL) ASC,COALESCE(c.volume,''::text) DESC,(({chapter}) IS NULL) ASC,COALESCE(({chapter}),0::numeric) DESC,(c.chapter IS NOT NULL) ASC,COALESCE(c.chapter,''::text) DESC"
            )
        );
        let sql = group_order(Profile::Native, "c", false);
        assert!(!sql.contains("COLLATE"));
        assert!(!sql.contains("lower("));
        assert!(!sql.contains("published_at"));
        assert!(!sql.contains("float"));
        assert!(!sql.contains("double"));
    }

    #[test]
    fn public_group_sql_uses_complete_ascii_decimal_and_utf16_arrays() {
        assert_eq!(
            numeric_label(Profile::Public, "c.volume"),
            r"CASE WHEN c.volume ~ E'\\A[0-9]+(?:[.][0-9]+)?\\Z' THEN c.volume::numeric END"
        );
        let terms = group_terms(Profile::Public, "c");
        assert_eq!(terms.len(), 8);
        assert_eq!(terms[2].expression, "(c.volume IS NULL)");
        assert_eq!(terms[6].expression, "(c.chapter IS NULL)");
        for index in [3, 7] {
            let sql = &terms[index].expression;
            assert!(sql.starts_with("ARRAY(SELECT __reader_units.unit "));
            assert!(sql.contains("__reader_codepoint.scalar>65535"));
            assert!(sql.contains("55296+((__reader_codepoint.scalar-65536)/1024)"));
            assert!(sql.contains("56320+((__reader_codepoint.scalar-65536)%1024)"));
            assert!(sql.contains("WITH ORDINALITY AS __reader_units(unit,part)"));
            assert!(sql.ends_with("ORDER BY __reader_chars.position,__reader_units.part)"));
            assert!(!sql.contains("COLLATE"));
        }
        assert!(!group_order(Profile::Public, "c", false).contains("float"));
    }

    #[test]
    fn mixed_direction_keyset_is_a_strict_null_safe_lexicographic_or() {
        let left = [
            Term::new("l.rank", Direction::Asc),
            Term::new("l.number", Direction::Desc),
            Term::new("l.raw", Direction::Desc),
        ];
        let right = [
            Term::new("r.rank", Direction::Asc),
            Term::new("r.number", Direction::Desc),
            Term::new("r.raw", Direction::Desc),
        ];
        assert_eq!(
            after(&left, &right, false),
            "(((l.rank) > (r.rank)) OR ((l.rank) IS NOT DISTINCT FROM (r.rank) AND (l.number) < (r.number)) OR ((l.rank) IS NOT DISTINCT FROM (r.rank) AND (l.number) IS NOT DISTINCT FROM (r.number) AND (l.raw) < (r.raw)))"
        );
        assert_eq!(
            after(&left, &right, true),
            "(((l.rank) < (r.rank)) OR ((l.rank) IS NOT DISTINCT FROM (r.rank) AND (l.number) > (r.number)) OR ((l.rank) IS NOT DISTINCT FROM (r.rank) AND (l.number) IS NOT DISTINCT FROM (r.number) AND (l.raw) > (r.raw)))"
        );
    }

    #[test]
    fn reversal_inverts_every_group_component_and_keeps_original_aliases() {
        for profile in [Profile::Native, Profile::Public] {
            let terms = group_terms(profile, "candidate");
            let reverse = group_order(profile, "candidate", true);
            assert_eq!(
                reverse,
                terms
                    .iter()
                    .map(|term| format!(
                        "{} {}",
                        term.expression,
                        match term.direction {
                            Direction::Asc => "DESC",
                            Direction::Desc => "ASC",
                        }
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            for reverse in [false, true] {
                let predicate = group_after(profile, "candidate", "anchor", reverse);
                assert_eq!(predicate.matches(" OR ").count(), 7);
                assert_eq!(predicate.matches("IS NOT DISTINCT FROM").count(), 28);
                assert!(predicate.contains("candidate.volume"));
                assert!(predicate.contains("anchor.volume"));
                assert!(predicate.contains("candidate.chapter"));
                assert!(predicate.contains("anchor.chapter"));
                assert!(!predicate.contains("ROW("));
            }
        }
    }

    #[test]
    fn edition_sql_preserves_different_native_and_public_ties() {
        let index = "CASE WHEN c.id ~ '^(0|[1-9][0-9]{0,9})$' THEN CASE WHEN c.id::bigint<=4294967294 THEN c.id::bigint END END";
        assert_eq!(
            edition_order(Profile::Native, "c"),
            format!("(({index}) IS NULL) ASC,COALESCE(({index}),0::bigint) ASC,c.id COLLATE \"C\" ASC")
        );
        assert_eq!(
            edition_order(Profile::Public, "c"),
            "c.published_at DESC,c.id ASC"
        );
        assert_eq!(
            edition_after(Profile::Public, "c", "a"),
            "(((c.published_at) < (a.published_at)) OR ((c.published_at) IS NOT DISTINCT FROM (a.published_at) AND (c.id) > (a.id)))"
        );
        let native = edition_after(Profile::Native, "c", "a");
        assert!(!native.contains("published_at"));
        assert_eq!(native.matches(" OR ").count(), 2);
        assert!(native.contains("(c.id COLLATE \"C\") > (a.id COLLATE \"C\")"));
    }

    // Independent, integer/string-only contract witnesses for the label
    // fixtures. They specify expected rows for future database integration
    // tests; they do not pretend that a Rust unit test executes PostgreSQL.
    // Native raw fixtures below use C collation, explicitly, while production
    // native SQL deliberately inherits the catalogue database collation.
    fn number_prefix(value: &str) -> Option<&str> {
        let bytes = value.as_bytes();
        let mut end = bytes.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if end == 0 {
            return None;
        }
        if bytes.get(end) == Some(&b'.')
            && bytes.get(end + 1).is_some_and(u8::is_ascii_digit)
        {
            end += 1;
            end += bytes[end..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
        }
        Some(&value[..end])
    }

    fn decimal_cmp(left: &str, right: &str) -> Ordering {
        let parts = |value: &str| {
            let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
            (integer.trim_start_matches('0').to_owned(), fraction.to_owned())
        };
        let (a, af) = parts(left);
        let (b, bf) = parts(right);
        a.len().cmp(&b.len()).then_with(|| a.cmp(&b)).then_with(|| {
            let size = af.len().max(bf.len());
            af.bytes()
                .chain(std::iter::repeat(b'0'))
                .take(size)
                .cmp(bf.bytes().chain(std::iter::repeat(b'0')).take(size))
        })
    }

    fn label_cmp(profile: Profile, left: Option<&str>, right: Option<&str>) -> Ordering {
        let number = |value: Option<&str>| {
            value.and_then(|value| {
                number_prefix(value).filter(|prefix| {
                    profile == Profile::Native || prefix.len() == value.len()
                }).map(str::to_owned)
            })
        };
        let a = number(left);
        let b = number(right);
        let numeric = match (&a, &b) {
            (Some(a), Some(b)) => decimal_cmp(b, a),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        };
        numeric.then_with(|| match (left, right) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => match profile {
                Profile::Native => Ordering::Less,
                Profile::Public => Ordering::Greater,
            },
            (Some(_), None) => match profile {
                Profile::Native => Ordering::Greater,
                Profile::Public => Ordering::Less,
            },
            (Some(a), Some(b)) => match profile {
                Profile::Native => b.cmp(a),
                Profile::Public => b.encode_utf16().cmp(a.encode_utf16()),
            },
        })
    }

    fn sorted<'a>(profile: Profile, labels: &[Option<&'a str>]) -> Vec<Option<&'a str>> {
        let mut result = labels.to_vec();
        result.sort_by(|left, right| label_cmp(profile, *left, *right));
        result
    }

    #[test]
    fn golden_prefix_special_and_nullable_labels_are_profile_specific() {
        let labels = [Some("special"), None, Some("1"), Some("2a"), Some("")];
        assert_eq!(
            sorted(Profile::Native, &labels),
            [Some("2a"), Some("1"), None, Some("special"), Some("")]
        );
        assert_eq!(
            sorted(Profile::Public, &labels),
            [Some("1"), Some("special"), Some("2a"), Some(""), None]
        );
        for value in [".2", "-2", "+2", " 2", "２", "2\n", "2.", "2a"] {
            assert!(number_prefix(value).is_none_or(|prefix| prefix != value));
        }
        assert_eq!(number_prefix("2.5a"), Some("2.5"));
        assert_eq!(number_prefix("2."), Some("2"));
    }

    #[test]
    fn golden_equivalent_decimals_keep_raw_group_identity() {
        for profile in [Profile::Native, Profile::Public] {
            assert_eq!(
                sorted(profile, &[Some("02"), Some("2"), Some("2.0")]),
                [Some("2.0"), Some("2"), Some("02")]
            );
            assert_ne!(label_cmp(profile, None, Some("")), Ordering::Equal);
            assert_ne!(label_cmp(profile, Some("02"), Some("2")), Ordering::Equal);
            assert_ne!(label_cmp(profile, Some("2.0"), Some("2")), Ordering::Equal);
        }
    }

    #[test]
    fn golden_numbers_above_two_to_53_and_tiny_fraction_steps_stay_exact() {
        let labels = [
            Some("9007199254740992"),
            Some("2.0000000000000000000000000001"),
            Some("9007199254740993"),
            Some("2.0000000000000000000000000002"),
            Some("2"),
        ];
        for profile in [Profile::Native, Profile::Public] {
            assert_eq!(
                sorted(profile, &labels),
                [
                    Some("9007199254740993"),
                    Some("9007199254740992"),
                    Some("2.0000000000000000000000000002"),
                    Some("2.0000000000000000000000000001"),
                    Some("2"),
                ]
            );
        }
        assert_eq!(decimal_cmp("000", "0.0000"), Ordering::Equal);
        assert_eq!(decimal_cmp("02.000", "2"), Ordering::Equal);
    }

    #[test]
    fn golden_public_surrogate_pairs_are_not_codepoint_or_utf8_order() {
        let labels = [Some("\u{10000}"), Some("\u{e000}"), Some("😀"), Some("\u{ffff}")];
        assert_eq!(
            sorted(Profile::Public, &labels),
            [Some("\u{ffff}"), Some("\u{e000}"), Some("😀"), Some("\u{10000}")]
        );
        assert_eq!(
            sorted(Profile::Native, &labels),
            [Some("😀"), Some("\u{10000}"), Some("\u{ffff}"), Some("\u{e000}")]
        );
        assert_eq!("\u{10000}".encode_utf16().collect::<Vec<_>>(), [55296, 56320]);
        assert_eq!("😀".encode_utf16().collect::<Vec<_>>(), [55357, 56832]);
        assert_eq!("\u{10ffff}".encode_utf16().collect::<Vec<_>>(), [56319, 57343]);
    }

    #[test]
    fn golden_volume_precedes_chapter_and_all_nullable_pairs_are_distinct() {
        let labels = [None, Some(""), Some("2a"), Some("1"), Some("02"), Some("2"), Some("2.0")];
        let groups = labels.iter().flat_map(|volume| {
            labels.iter().map(move |chapter| (*volume, *chapter))
        }).collect::<Vec<_>>();
        for profile in [Profile::Native, Profile::Public] {
            let compare = |a: &(Option<&str>, Option<&str>), b: &(Option<&str>, Option<&str>)| {
                label_cmp(profile, a.0, b.0).then_with(|| label_cmp(profile, a.1, b.1))
            };
            assert_eq!(compare(&(Some("2"), Some("0")), &(Some("1"), Some("999"))), Ordering::Less);
            let mut ordered = groups.clone();
            ordered.sort_by(compare);
            for (index, anchor) in ordered.iter().enumerate() {
                assert_eq!(compare(anchor, anchor), Ordering::Equal);
                let after = ordered.iter().filter(|candidate| compare(candidate, anchor).is_gt()).copied().collect::<Vec<_>>();
                assert_eq!(after, ordered[index + 1..]);
                let mut before = ordered.iter().filter(|candidate| compare(candidate, anchor).is_lt()).copied().collect::<Vec<_>>();
                before.reverse();
                assert_eq!(before, ordered[..index].iter().rev().copied().collect::<Vec<_>>());
            }
            assert!(ordered.windows(2).all(|pair| compare(&pair[0], &pair[1]).is_lt()));
        }
    }

    fn array_index(id: &str) -> Option<u32> {
        id.parse::<u32>().ok().filter(|value| {
            *value != u32::MAX && value.to_string() == id
        })
    }

    #[test]
    fn golden_native_editions_follow_btree_then_object_entries_property_order() {
        let mut ids = ["10", "02", "4294967295", "2", "4294967294", "0", "00", "a", "\u{10000}", "\u{e000}"];
        ids.sort_by(|left, right| match (array_index(left), array_index(right)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => left.cmp(right),
        });
        assert_eq!(ids, ["0", "2", "10", "4294967294", "00", "02", "4294967295", "a", "\u{e000}", "\u{10000}"]);
        for id in ["", "02", "00", "-0", "+2", "2.0", "2e0", "4294967295", "4294967296", "99999999999999999999999999999999"] {
            assert_eq!(array_index(id), None);
        }
    }

    #[test]
    fn golden_public_editions_use_exact_date_then_database_id_not_numeric_property_order() {
        // Integer microseconds witness the timestamp ordering. Equal dates use
        // C-collated native IDs here; SQL keeps the actual database collation.
        let mut rows = [(1_700_000_000_000_001_i64, "2"), (1_700_000_000_000_001, "10"), (1_700_000_000_000_002, "02")];
        rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        assert_eq!(rows.map(|(_, id)| id), ["02", "10", "2"]);
    }
}
