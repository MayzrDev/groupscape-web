use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};

use crate::drop_rates::slugify_npc_name;

/// Mirrors `site/src/data/boss-levels.js`'s `BOSS_COMBAT_LEVELS` - kept in sync manually. Only
/// used by the Loot Log search grammar's numeric clause (see [`numeric_clause_matches`]), which
/// can test a query token against a kill source's combat level alongside item value/quantity.
static BOSS_COMBAT_LEVELS: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| {
    HashMap::from([
        ("vorkath", 732),
        ("zulrah", 725),
        ("cerberus", 318),
        ("general_graardor", 624),
        ("kril_tsutsaroth", 650),
        ("commander_zilyana", 596),
        ("kreearra", 580),
        ("alchemical_hydra", 426),
        ("king_black_dragon", 276),
        ("giant_mole", 230),
        ("kalphite_queen", 333),
        ("abyssal_sire", 350),
        ("thermonuclear_smoke_devil", 301),
        ("vetion", 454),
        ("callisto", 470),
        ("venenatis", 464),
        ("chaos_elemental", 305),
        ("scorpia", 225),
        ("zalcano", 336),
        ("nightmare_of_ashihama", 814),
        ("phantom_muspah", 741),
        ("corporeal_beast", 785),
        ("tztok_jad", 702),
        ("tzkal_zuk", 1400),
        ("sarachnis", 318),
        ("skotizo", 321),
        ("obor", 106),
        ("bryophyta", 128),
        ("dagannoth_rex", 303),
        ("dagannoth_prime", 303),
        ("dagannoth_supreme", 303),
        ("duke_sucellus", 758),
        ("the_leviathan", 798),
        ("the_whisperer", 791),
        ("vardorvis", 784),
        ("amoxliatl", 263),
        ("the_hueycoatl", 642),
        ("yama", 1238),
        ("araxxor", 890),
        ("artio", 320),
        ("calvarion", 264),
        ("sol_heredit", 1563),
        ("doom_of_mokhaiotl", 558),
        ("scurrius", 200),
        ("hespori", 284),
        ("chaos_fanatic", 202),
        ("crazy_archaeologist", 204),
        ("deranged_archaeologist", 276),
    ])
});

/// Fixed combat level for a boss with one canonical level (see `BOSS_COMBAT_LEVELS`'s doc comment
/// in the JS mirror for why minigames/multi-NPC encounters are omitted). `None` for anything not
/// in the curated table - the search grammar's level clause simply never matches those sources.
pub fn combat_level(source_name: &str) -> Option<i32> {
    BOSS_COMBAT_LEVELS.get(slugify_npc_name(source_name).as_str()).copied()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericOp {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Copy, Debug)]
pub struct NumericClause {
    op: NumericOp,
    value: f64,
}

/// Parses one search token as a numeric expression: optional `>`/`<`/`>=`/`<=`/`=` prefix (a bare
/// number is an exact match), comma-formatted or plain digits, optional `k`/`m`/`b` suffix
/// multiplier (`2.5m` = 2,500,000). Returns `None` for anything that isn't a number at all (e.g.
/// "zulrah"), which the caller then falls back to treating as a plain text token.
pub fn parse_numeric_clause(token: &str) -> Option<NumericClause> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    let (op, rest) = if let Some(r) = token.strip_prefix(">=") {
        (NumericOp::Gte, r)
    } else if let Some(r) = token.strip_prefix("<=") {
        (NumericOp::Lte, r)
    } else if let Some(r) = token.strip_prefix('>') {
        (NumericOp::Gt, r)
    } else if let Some(r) = token.strip_prefix('<') {
        (NumericOp::Lt, r)
    } else if let Some(r) = token.strip_prefix('=') {
        (NumericOp::Eq, r)
    } else {
        (NumericOp::Eq, token)
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    let lower_last = rest.chars().last()?.to_ascii_lowercase();
    let (number_part, multiplier) = match lower_last {
        'k' => (&rest[..rest.len() - 1], 1_000.0),
        'm' => (&rest[..rest.len() - 1], 1_000_000.0),
        'b' => (&rest[..rest.len() - 1], 1_000_000_000.0),
        _ => (rest, 1.0),
    };
    let cleaned: String = number_part.chars().filter(|c| *c != ',').collect();
    if cleaned.is_empty() {
        return None;
    }
    let base: f64 = cleaned.parse().ok()?;
    Some(NumericClause {
        op,
        value: base * multiplier,
    })
}

/// Whether `candidate` satisfies a parsed numeric clause. Compared as floats with a sub-1 epsilon
/// since `candidate` is always really an integer (gp value, quantity, or combat level) that may
/// have passed through a `k`/`m`/`b` multiplier on the clause side.
pub fn numeric_clause_matches(clause: &NumericClause, candidate: i64) -> bool {
    let candidate = candidate as f64;
    match clause.op {
        NumericOp::Eq => (candidate - clause.value).abs() < 0.5,
        NumericOp::Gt => candidate > clause.value,
        NumericOp::Gte => candidate >= clause.value,
        NumericOp::Lt => candidate < clause.value,
        NumericOp::Lte => candidate <= clause.value,
    }
}

/// Same comparison as `numeric_clause_matches`, for probability-valued candidates (drop rates,
/// always well under 1.0) where the value clause's sub-1 epsilon would be meaningless.
pub fn numeric_clause_matches_f64(clause: &NumericClause, candidate: f64) -> bool {
    match clause.op {
        NumericOp::Eq => (candidate - clause.value).abs() < 1e-9,
        NumericOp::Gt => candidate > clause.value,
        NumericOp::Gte => candidate >= clause.value,
        NumericOp::Lt => candidate < clause.value,
        NumericOp::Lte => candidate <= clause.value,
    }
}

/// Parses a drop-rate search clause like `<1/5000` or `1/508`: the same optional comparison
/// prefix as `parse_numeric_clause`, followed by a curated `"n/d"` rate (see
/// `crate::drop_rates::probability`), compared as a plain probability - so `<1/5000` reads
/// naturally as "rarer than 1 in 5000" (a smaller probability IS rarer, no inversion needed).
pub fn parse_drop_rate_clause(token: &str) -> Option<NumericClause> {
    let token = token.trim();
    let (op, rest) = if let Some(r) = token.strip_prefix(">=") {
        (NumericOp::Gte, r)
    } else if let Some(r) = token.strip_prefix("<=") {
        (NumericOp::Lte, r)
    } else if let Some(r) = token.strip_prefix('>') {
        (NumericOp::Gt, r)
    } else if let Some(r) = token.strip_prefix('<') {
        (NumericOp::Lt, r)
    } else if let Some(r) = token.strip_prefix('=') {
        (NumericOp::Eq, r)
    } else {
        (NumericOp::Eq, token)
    };
    let value = crate::drop_rates::probability(rest.trim())?;
    Some(NumericClause { op, value })
}

/// Rarity tiers as stored in `content/drop_rates.json` (see `crate::drop_rates::rarity_rank`).
/// `"rare"` deliberately also matches the `very_rare` tier (rare-or-rarer) since that reads more
/// naturally as "show me the rare stuff" than requiring an exact tier match.
pub fn matches_rarity_keyword(keyword: &str, rarity: Option<&str>) -> bool {
    let Some(rarity) = rarity else { return false };
    match keyword {
        "rare" => rarity == "rare" || rarity == "very_rare",
        "very_rare" => rarity == "very_rare",
        "uncommon" => rarity == "uncommon",
        "common" => rarity == "common",
        _ => false,
    }
}

/// Recognizes a search token as a rarity keyword (case-insensitive; `veryrare`/`very_rare` both
/// accepted since `&&`/`and` grouping already covers the "two separate words" case some other
/// way). Returns the canonical tier name `matches_rarity_keyword` expects, or `None`.
pub fn rarity_keyword(token: &str) -> Option<&'static str> {
    match token.to_lowercase().as_str() {
        "rare" => Some("rare"),
        "veryrare" | "very_rare" => Some("very_rare"),
        "uncommon" => Some("uncommon"),
        "common" => Some("common"),
        _ => None,
    }
}

/// Whether a plain-text search token names a Loot Log category by its `source_type` ("kill",
/// "chest", "clue") rather than a literal name - lets e.g. `chest && >5m` work generically instead
/// of only matching kills/chests whose actual name happens to contain the word.
pub fn matches_category_keyword(token: &str, source_type: &str) -> bool {
    matches!(
        (token, source_type),
        ("kill" | "kills", "kill") | ("chest" | "chests", "chest") | ("clue" | "clues", "clue")
    )
}

/// Kill count ("kc") is a farming-session concept - the number of events merged into one
/// loot-log-group card client-side (see loot-log-group.js's `countLabel`) - that only exists
/// after the client merges raw events, so it can't be evaluated per-event here the way
/// value/quantity/level clauses are. [`extract_kill_count_clause`] exists purely so a kc token
/// doesn't get misread as a literal text/value token by [`crate::authed::build_matching_loot_log_event`]
/// (which would wrongly exclude every event in the group); the actual kc filtering happens
/// client-side in loot-log-page.js's `parseKillCountClauses`, which mirrors this grammar and must
/// be kept in sync with it.
const KILL_COUNT_WORDS: [&str; 3] = ["kc", "kills", "kill"];

/// Finds a kill-count clause within one AND-group of tokens, either attached to the number
/// (`10kc`, like the `k`/`m`/`b` multiplier suffixes) or as its own following word (`10 kills`),
/// and returns the clause plus the group's remaining tokens with it removed. `None` if the group
/// has no kill-count clause at all.
pub fn extract_kill_count_clause(tokens: &[String]) -> (Option<NumericClause>, Vec<String>) {
    for (i, token) in tokens.iter().enumerate() {
        let lower = token.to_lowercase();
        for word in KILL_COUNT_WORDS {
            if lower.len() > word.len() && lower.ends_with(word) {
                if let Some(clause) = parse_numeric_clause(&token[..token.len() - word.len()]) {
                    let mut rest = tokens.to_vec();
                    rest.remove(i);
                    return (Some(clause), rest);
                }
            }
        }
    }
    for i in 0..tokens.len().saturating_sub(1) {
        if let Some(clause) = parse_numeric_clause(&tokens[i]) {
            if KILL_COUNT_WORDS.iter().any(|w| tokens[i + 1].eq_ignore_ascii_case(w)) {
                let mut rest = tokens.to_vec();
                rest.remove(i + 1);
                rest.remove(i);
                return (Some(clause), rest);
            }
        }
    }
    (None, tokens.to_vec())
}

/// Splits a raw Loot Log search string into AND-separated condition groups: `&&` and the
/// standalone word `and` (case-insensitive) both act as separators between groups, and the
/// caller (`build_matching_loot_log_event`) requires every group to match. Plain whitespace is
/// deliberately NOT a separator - NPC and item names routinely contain spaces (e.g. "General
/// Graardor", "Zamorak brew"), and splitting a name's words into separate AND'd groups would
/// break substring matching against them. Each returned group is itself whitespace-tokenized,
/// preserving the existing OR-across-tokens matching within a group.
pub fn split_search_groups(search: &str) -> Vec<Vec<String>> {
    let spaced = search.replace("&&", " && ");
    let mut groups: Vec<Vec<String>> = vec![Vec::new()];
    for word in spaced.split_whitespace() {
        if word == "&&" || word.eq_ignore_ascii_case("and") {
            if !groups.last().unwrap().is_empty() {
                groups.push(Vec::new());
            }
            continue;
        }
        groups.last_mut().unwrap().push(word.to_string());
    }
    groups.retain(|g| !g.is_empty());
    groups
}

/// A half-open time window (`after` inclusive, `before` exclusive - matching `get_loot_log`'s
/// existing `occurred_at < before` cursor convention) resolved from date tokens in a Loot Log
/// search string. `None` on either side means "no bound from that side" - a search with no date
/// tokens at all resolves to `DateBounds::default()` (unbounded).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateBounds {
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
}

impl DateBounds {
    /// AND-combines two bounds into their intersection (the narrower window) - `after` clauses
    /// take the latest (max) start, `before` clauses take the earliest (min) end. A search with
    /// contradictory date clauses (e.g. "today and yesterday") intersects down to an empty/
    /// inverted window, which the caller's scan loop then naturally yields zero events for.
    pub fn intersect(self, other: DateBounds) -> DateBounds {
        DateBounds {
            after: match (self.after, other.after) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            },
            before: match (self.before, other.before) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
        }
    }
}

/// Converts a viewer-local naive calendar instant (a wall-clock date/time in the viewer's own
/// timezone, with no timezone info of its own) to the UTC instant it actually represents, given
/// the viewer's UTC offset in minutes (`Date.prototype.getTimezoneOffset()` - positive when local
/// time is behind UTC). `utc = local + offset`.
fn local_midnight_to_utc(date: NaiveDate, tz_offset_minutes: i32) -> DateTime<Utc> {
    let naive = date.and_hms_opt(0, 0, 0).expect("midnight is always a valid time");
    DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc) + Duration::minutes(tz_offset_minutes as i64)
}

/// The viewer-local calendar date `now_utc` falls on, given the viewer's UTC offset in minutes -
/// the inverse of `local_midnight_to_utc`: `local = utc - offset`.
fn local_date(now_utc: DateTime<Utc>, tz_offset_minutes: i32) -> NaiveDate {
    (now_utc - Duration::minutes(tz_offset_minutes as i64)).date_naive()
}

/// One local calendar day as a half-open `[start, start-of-next-day)` window.
fn day_bounds(date: NaiveDate, tz_offset_minutes: i32) -> DateBounds {
    DateBounds {
        after: Some(local_midnight_to_utc(date, tz_offset_minutes)),
        before: Some(local_midnight_to_utc(date + Duration::days(1), tz_offset_minutes)),
    }
}

/// Parses a strict `YYYY-MM-DD` token (the only specific-date format accepted - unambiguous
/// across locales, unlike `MM/DD/YYYY` vs `DD/MM/YYYY`).
fn parse_iso_date(token: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(token.trim(), "%Y-%m-%d").ok()
}

/// Parses one comparison-prefixed or bare date token (`2026-05-05`, `>2026-05-05`,
/// `>=2026-05-05`, `<2026-05-05`, `<=2026-05-05`) into a bound. A bare date or `>`/`>=` all mean
/// "from this day onward, inclusive" (the day's start); `<`/`<=` both mean "through the end of
/// this day, inclusive" (the *next* day's start, as the exclusive upper bound) - `>`/`>=` and
/// `<`/`<=` are deliberately equivalent for dates, since a day is a bucket, not a point.
fn parse_date_bound_token(token: &str, tz_offset_minutes: i32) -> Option<DateBounds> {
    let token = token.trim();
    // `None` = bare date, no comparison prefix at all - that's `day_bounds`'s single-day window,
    // not an open-ended one; `Some(is_upper)` says which side the prefix bounds.
    let (op, rest) = if let Some(r) = token.strip_prefix(">=") {
        (Some(false), r)
    } else if let Some(r) = token.strip_prefix("<=") {
        (Some(true), r)
    } else if let Some(r) = token.strip_prefix('>') {
        (Some(false), r)
    } else if let Some(r) = token.strip_prefix('<') {
        (Some(true), r)
    } else {
        (None, token)
    };
    let date = parse_iso_date(rest)?;
    match op {
        None => Some(day_bounds(date, tz_offset_minutes)),
        Some(true) => Some(DateBounds {
            after: None,
            before: Some(local_midnight_to_utc(date + Duration::days(1), tz_offset_minutes)),
        }),
        Some(false) => Some(DateBounds {
            after: Some(local_midnight_to_utc(date, tz_offset_minutes)),
            before: None,
        }),
    }
}

/// Parses a `YYYY-MM-DD..YYYY-MM-DD` range token - both ends inclusive calendar days.
fn parse_dot_range_token(token: &str, tz_offset_minutes: i32) -> Option<DateBounds> {
    let (start, end) = token.trim().split_once("..")?;
    let start = parse_iso_date(start)?;
    let end = parse_iso_date(end)?;
    Some(DateBounds {
        after: Some(local_midnight_to_utc(start, tz_offset_minutes)),
        before: Some(local_midnight_to_utc(end + Duration::days(1), tz_offset_minutes)),
    })
}

/// `this week`/`this hour`/`this month` (case-insensitive, exactly these two tokens). Week starts
/// Monday. The window runs from the unit's start through `now` (open-ended upper bound, since
/// "this week" includes events still happening).
fn parse_this_unit(tokens: &[&str], now_utc: DateTime<Utc>, tz_offset_minutes: i32) -> Option<DateBounds> {
    if tokens.len() != 2 || !tokens[0].eq_ignore_ascii_case("this") {
        return None;
    }
    let today = local_date(now_utc, tz_offset_minutes);
    let after = match tokens[1].to_lowercase().as_str() {
        "hour" => {
            let local_now = now_utc - Duration::minutes(tz_offset_minutes as i64);
            let naive = local_now
                .date_naive()
                .and_hms_opt(local_now.hour(), 0, 0)
                .expect("top of the hour is always valid");
            DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc) + Duration::minutes(tz_offset_minutes as i64)
        }
        "week" => {
            let days_since_monday = today.weekday().num_days_from_monday();
            local_midnight_to_utc(today - Duration::days(days_since_monday as i64), tz_offset_minutes)
        }
        "month" => {
            let start_of_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)?;
            local_midnight_to_utc(start_of_month, tz_offset_minutes)
        }
        _ => return None,
    };
    Some(DateBounds {
        after: Some(after),
        before: None,
    })
}

/// `last N <unit>` (case-insensitive, exactly three tokens) - a rolling window from `N` units
/// before `now` through `now`. `unit` accepts singular/plural: hour(s), day(s), week(s), month(s).
fn parse_last_n_unit(tokens: &[&str], now_utc: DateTime<Utc>) -> Option<DateBounds> {
    if tokens.len() != 3 || !tokens[0].eq_ignore_ascii_case("last") {
        return None;
    }
    let n: i64 = tokens[1].parse().ok()?;
    if n <= 0 {
        return None;
    }
    let duration = match tokens[2].to_lowercase().as_str() {
        "hour" | "hours" => Duration::hours(n),
        "day" | "days" => Duration::days(n),
        "week" | "weeks" => Duration::weeks(n),
        "month" | "months" => Duration::days(n * 30),
        _ => return None,
    };
    Some(DateBounds {
        after: Some(now_utc - duration),
        before: None,
    })
}

/// Tries to match a date clause starting at `tokens[start]`, longest pattern first (`last N
/// <unit>` is 3 tokens, `this <unit>` is 2, everything else is 1) so e.g. "this" isn't matched as
/// a stray single token before the 2-token form gets a chance. Returns the resolved bound and how
/// many tokens it consumed, or `None` if nothing at `start` looks like a date clause.
fn try_match_date_clause(
    tokens: &[String],
    start: usize,
    now_utc: DateTime<Utc>,
    tz_offset_minutes: i32,
) -> Option<(DateBounds, usize)> {
    if start + 3 <= tokens.len() {
        let window: Vec<&str> = tokens[start..start + 3].iter().map(String::as_str).collect();
        if let Some(bounds) = parse_last_n_unit(&window, now_utc) {
            return Some((bounds, 3));
        }
    }
    if start + 2 <= tokens.len() {
        let window: Vec<&str> = tokens[start..start + 2].iter().map(String::as_str).collect();
        if let Some(bounds) = parse_this_unit(&window, now_utc, tz_offset_minutes) {
            return Some((bounds, 2));
        }
    }
    let token = &tokens[start];
    let lower = token.to_lowercase();
    if lower == "today" {
        return Some((day_bounds(local_date(now_utc, tz_offset_minutes), tz_offset_minutes), 1));
    }
    if lower == "yesterday" {
        return Some((
            day_bounds(local_date(now_utc, tz_offset_minutes) - Duration::days(1), tz_offset_minutes),
            1,
        ));
    }
    if let Some(bounds) = parse_dot_range_token(token, tz_offset_minutes) {
        return Some((bounds, 1));
    }
    if let Some(bounds) = parse_date_bound_token(token, tz_offset_minutes) {
        return Some((bounds, 1));
    }
    None
}

/// Repeatedly extracts every date clause found in one AND-group's tokens, intersecting them all
/// together (date clauses always AND within a group, unlike the general OR-across-tokens rule -
/// "&gt;2026-05-01 &lt;2026-05-05" reads as a range, not "before X OR after Y"). Returns the
/// combined bounds (`DateBounds::default()` if the group had no date tokens at all) plus the
/// group's remaining non-date tokens, which still form a normal AND-required text/value group if
/// non-empty (mirrors `extract_kill_count_clause`'s carve-out: a group left empty after
/// extraction imposes no further per-event constraint, since the bound alone already covers it).
pub fn extract_date_clauses(
    tokens: &[String],
    now_utc: DateTime<Utc>,
    tz_offset_minutes: i32,
) -> (DateBounds, Vec<String>) {
    let mut bounds = DateBounds::default();
    let mut rest = tokens.to_vec();
    let mut i = 0;
    while i < rest.len() {
        if let Some((found, consumed)) = try_match_date_clause(&rest, i, now_utc, tz_offset_minutes) {
            bounds = bounds.intersect(found);
            rest.drain(i..i + consumed);
        } else {
            i += 1;
        }
    }
    (bounds, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_number_as_exact() {
        let clause = parse_numeric_clause("100000").unwrap();
        assert!(numeric_clause_matches(&clause, 100_000));
        assert!(!numeric_clause_matches(&clause, 100_001));
    }

    #[test]
    fn parses_comma_formatted_numbers() {
        let clause = parse_numeric_clause("100,000").unwrap();
        assert!(numeric_clause_matches(&clause, 100_000));
    }

    #[test]
    fn parses_suffix_multipliers() {
        let clause = parse_numeric_clause(">1m").unwrap();
        assert!(numeric_clause_matches(&clause, 1_000_001));
        assert!(!numeric_clause_matches(&clause, 1_000_000));

        let clause = parse_numeric_clause("2.5m").unwrap();
        assert!(numeric_clause_matches(&clause, 2_500_000));

        let clause = parse_numeric_clause("<100k").unwrap();
        assert!(numeric_clause_matches(&clause, 99_999));
        assert!(!numeric_clause_matches(&clause, 100_000));
    }

    #[test]
    fn parses_comparison_prefixes() {
        assert!(numeric_clause_matches(&parse_numeric_clause(">=5").unwrap(), 5));
        assert!(numeric_clause_matches(&parse_numeric_clause("<=5").unwrap(), 5));
        assert!(!numeric_clause_matches(&parse_numeric_clause(">5").unwrap(), 5));
    }

    #[test]
    fn non_numeric_tokens_return_none() {
        assert!(parse_numeric_clause("zulrah").is_none());
        assert!(parse_numeric_clause("master clue").is_none());
        assert!(parse_numeric_clause("").is_none());
    }

    #[test]
    fn looks_up_known_boss_levels() {
        assert_eq!(combat_level("Vorkath"), Some(732));
        assert_eq!(combat_level("Zulrah"), Some(725));
        assert_eq!(combat_level("Some Random Npc"), None);
    }

    fn groups(items: &[&[&str]]) -> Vec<Vec<String>> {
        items.iter().map(|g| g.iter().map(|s| s.to_string()).collect()).collect()
    }

    #[test]
    fn plain_whitespace_stays_one_group() {
        assert_eq!(split_search_groups("general graardor >1m"), groups(&[&["general", "graardor", ">1m"]]));
    }

    #[test]
    fn double_ampersand_splits_into_groups() {
        assert_eq!(split_search_groups("vorkath && >1m"), groups(&[&["vorkath"], &[">1m"]]));
        assert_eq!(split_search_groups("vorkath&&>1m"), groups(&[&["vorkath"], &[">1m"]]));
    }

    #[test]
    fn standalone_and_word_splits_case_insensitively() {
        assert_eq!(split_search_groups("vorkath AND >1m"), groups(&[&["vorkath"], &[">1m"]]));
    }

    #[test]
    fn and_as_a_substring_of_a_word_is_not_a_separator() {
        // "Sand crab" shouldn't be torn apart just because it contains "and".
        assert_eq!(split_search_groups("sand crab"), groups(&[&["sand", "crab"]]));
    }

    #[test]
    fn empty_and_separator_only_input_yields_no_groups() {
        assert!(split_search_groups("").is_empty());
        assert!(split_search_groups("&& and &&").is_empty());
    }

    fn strs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn extracts_attached_kc_suffix() {
        let (clause, rest) = extract_kill_count_clause(&strs(&[">10kc"]));
        assert!(numeric_clause_matches(&clause.unwrap(), 11));
        assert!(rest.is_empty());
    }

    #[test]
    fn extracts_attached_kills_suffix() {
        let (clause, rest) = extract_kill_count_clause(&strs(&["5kills"]));
        assert!(numeric_clause_matches(&clause.unwrap(), 5));
        assert!(rest.is_empty());
    }

    #[test]
    fn extracts_two_token_kills_form() {
        let (clause, rest) = extract_kill_count_clause(&strs(&["vorkath", ">10", "kills"]));
        assert!(numeric_clause_matches(&clause.unwrap(), 11));
        assert_eq!(rest, strs(&["vorkath"]));
    }

    #[test]
    fn ten_k_alone_is_not_a_kill_count_clause() {
        // "10k" is the existing thousands multiplier, not a kill-count suffix.
        let (clause, rest) = extract_kill_count_clause(&strs(&[">10k"]));
        assert!(clause.is_none());
        assert_eq!(rest, strs(&[">10k"]));
    }

    #[test]
    fn no_kill_count_clause_returns_tokens_unchanged() {
        let (clause, rest) = extract_kill_count_clause(&strs(&["vorkath", ">1m"]));
        assert!(clause.is_none());
        assert_eq!(rest, strs(&["vorkath", ">1m"]));
    }

    #[test]
    fn rarity_keyword_recognizes_known_tiers_case_insensitively() {
        assert_eq!(rarity_keyword("RARE"), Some("rare"));
        assert_eq!(rarity_keyword("veryrare"), Some("very_rare"));
        assert_eq!(rarity_keyword("very_rare"), Some("very_rare"));
        assert_eq!(rarity_keyword("uncommon"), Some("uncommon"));
        assert_eq!(rarity_keyword("common"), Some("common"));
        assert_eq!(rarity_keyword("vorkath"), None);
    }

    #[test]
    fn rare_keyword_also_matches_very_rare_tier() {
        assert!(matches_rarity_keyword("rare", Some("rare")));
        assert!(matches_rarity_keyword("rare", Some("very_rare")));
        assert!(!matches_rarity_keyword("rare", Some("uncommon")));
        assert!(!matches_rarity_keyword("very_rare", Some("rare")));
        assert!(!matches_rarity_keyword("rare", None));
    }

    #[test]
    fn parses_drop_rate_clause_as_a_probability_comparison() {
        let clause = parse_drop_rate_clause("<1/5000").unwrap();
        assert!(numeric_clause_matches_f64(&clause, 1.0 / 10_000.0));
        assert!(!numeric_clause_matches_f64(&clause, 1.0 / 100.0));

        let clause = parse_drop_rate_clause("1/508").unwrap();
        assert!(numeric_clause_matches_f64(&clause, 1.0 / 508.0));
    }

    #[test]
    fn drop_rate_clause_rejects_non_fraction_tokens() {
        assert!(parse_drop_rate_clause("vorkath").is_none());
        assert!(parse_drop_rate_clause(">1m").is_none());
    }

    #[test]
    fn category_keyword_matches_source_type_not_literal_text() {
        assert!(matches_category_keyword("chest", "chest"));
        assert!(matches_category_keyword("chests", "chest"));
        assert!(matches_category_keyword("kills", "kill"));
        assert!(!matches_category_keyword("chest", "kill"));
        assert!(!matches_category_keyword("vorkath", "kill"));
    }

    use chrono::TimeZone;

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, mi, 0).unwrap()
    }

    // 2026-05-15 is a Friday. `now` is 02:00 UTC, and every test below uses a -300 (EST, UTC-5)
    // offset, so the viewer's local clock reads 2026-05-14 21:00 - a full calendar day behind
    // UTC, deliberately exercising the cross-day-boundary math rather than a same-day offset that
    // would pass even with the timezone conversion silently doing nothing.
    const NOW: fn() -> DateTime<Utc> = || dt(2026, 5, 15, 2, 0);
    const EST_OFFSET: i32 = 300;

    #[test]
    fn today_resolves_to_viewer_local_calendar_day() {
        let (bounds, rest) = extract_date_clauses(&strs(&["today"]), NOW(), EST_OFFSET);
        assert!(rest.is_empty());
        assert_eq!(bounds.after, Some(dt(2026, 5, 14, 5, 0))); // 2026-05-14 00:00 EST = 05:00 UTC
        assert_eq!(bounds.before, Some(dt(2026, 5, 15, 5, 0))); // 2026-05-15 00:00 EST = 05:00 UTC
    }

    #[test]
    fn yesterday_is_one_local_day_before_today() {
        let (bounds, _) = extract_date_clauses(&strs(&["yesterday"]), NOW(), EST_OFFSET);
        assert_eq!(bounds.after, Some(dt(2026, 5, 13, 5, 0)));
        assert_eq!(bounds.before, Some(dt(2026, 5, 14, 5, 0)));
    }

    #[test]
    fn bare_iso_date_is_the_whole_local_day() {
        let (bounds, _) = extract_date_clauses(&strs(&["2026-05-01"]), NOW(), EST_OFFSET);
        assert_eq!(bounds.after, Some(dt(2026, 5, 1, 5, 0)));
        assert_eq!(bounds.before, Some(dt(2026, 5, 2, 5, 0)));
    }

    #[test]
    fn greater_than_date_is_inclusive_from_that_day() {
        let (gt, _) = extract_date_clauses(&strs(&[">2026-05-01"]), NOW(), EST_OFFSET);
        let (gte, _) = extract_date_clauses(&strs(&[">=2026-05-01"]), NOW(), EST_OFFSET);
        // `>` and `>=` are deliberately equivalent for dates - both mean "from this day onward".
        assert_eq!(gt.after, Some(dt(2026, 5, 1, 5, 0)));
        assert_eq!(gt.after, gte.after);
        assert!(gt.before.is_none());
    }

    #[test]
    fn less_than_date_is_inclusive_through_end_of_that_day() {
        let (lt, _) = extract_date_clauses(&strs(&["<2026-05-05"]), NOW(), EST_OFFSET);
        let (lte, _) = extract_date_clauses(&strs(&["<=2026-05-05"]), NOW(), EST_OFFSET);
        assert_eq!(lt.before, Some(dt(2026, 5, 6, 5, 0))); // start of the NEXT day - 05-05 is included
        assert_eq!(lt.before, lte.before);
        assert!(lt.after.is_none());
    }

    #[test]
    fn dot_range_is_inclusive_on_both_ends() {
        let (bounds, rest) = extract_date_clauses(&strs(&["2026-05-01..2026-05-05"]), NOW(), EST_OFFSET);
        assert!(rest.is_empty());
        assert_eq!(bounds.after, Some(dt(2026, 5, 1, 5, 0)));
        assert_eq!(bounds.before, Some(dt(2026, 5, 6, 5, 0)));
    }

    #[test]
    fn this_hour_starts_at_the_local_top_of_the_hour() {
        let now = dt(2026, 5, 15, 2, 45); // 21:45 EST
        let (bounds, _) = extract_date_clauses(&strs(&["this", "hour"]), now, EST_OFFSET);
        assert_eq!(bounds.after, Some(dt(2026, 5, 15, 2, 0))); // 21:00 EST = 02:00 UTC
        assert!(bounds.before.is_none());
    }

    #[test]
    fn this_week_starts_monday() {
        // 2026-05-14 (local date for NOW()) is a Thursday - Monday of that week is 2026-05-11.
        let (bounds, rest) = extract_date_clauses(&strs(&["this", "week"]), NOW(), EST_OFFSET);
        assert!(rest.is_empty());
        assert_eq!(bounds.after, Some(dt(2026, 5, 11, 5, 0))); // 2026-05-11 00:00 EST = 05:00 UTC
        assert!(bounds.before.is_none());
    }

    #[test]
    fn this_month_starts_on_the_1st() {
        let (bounds, _) = extract_date_clauses(&strs(&["this", "month"]), NOW(), EST_OFFSET);
        assert_eq!(bounds.after, Some(dt(2026, 5, 1, 5, 0))); // 2026-05-01 00:00 EST = 05:00 UTC
    }

    #[test]
    fn last_n_days_is_a_rolling_window_ending_now() {
        let (bounds, rest) = extract_date_clauses(&strs(&["last", "3", "days"]), NOW(), EST_OFFSET);
        assert!(rest.is_empty());
        assert_eq!(bounds.after, Some(NOW() - Duration::days(3)));
        assert!(bounds.before.is_none());
    }

    #[test]
    fn last_n_hours_singular_and_plural_both_work() {
        let (a, _) = extract_date_clauses(&strs(&["last", "24", "hours"]), NOW(), EST_OFFSET);
        let (b, _) = extract_date_clauses(&strs(&["last", "1", "hour"]), NOW(), EST_OFFSET);
        assert_eq!(a.after, Some(NOW() - Duration::hours(24)));
        assert_eq!(b.after, Some(NOW() - Duration::hours(1)));
    }

    #[test]
    fn two_date_tokens_in_one_group_and_together_into_a_range_without_a_separator() {
        // Unlike ordinary same-group tokens (which OR), date tokens always AND - this is the only
        // way to spell an explicit two-sided range using the >/< prefix form.
        let (bounds, rest) =
            extract_date_clauses(&strs(&[">2026-05-01", "<2026-05-05"]), NOW(), EST_OFFSET);
        assert!(rest.is_empty());
        assert_eq!(bounds.after, Some(dt(2026, 5, 1, 5, 0)));
        assert_eq!(bounds.before, Some(dt(2026, 5, 6, 5, 0)));
    }

    #[test]
    fn contradictory_date_clauses_intersect_to_an_empty_window() {
        // "today and yesterday" can never both be true - intersecting collapses to after >= before,
        // which the caller's scan loop naturally yields zero events for rather than needing a
        // special case here.
        let (bounds, _) = extract_date_clauses(&strs(&["today", "yesterday"]), NOW(), EST_OFFSET);
        assert!(bounds.after.unwrap() >= bounds.before.unwrap());
    }

    #[test]
    fn mixed_group_keeps_non_date_tokens_and_extracts_the_date_bound() {
        let (bounds, rest) = extract_date_clauses(&strs(&["vorkath", "today"]), NOW(), EST_OFFSET);
        assert_eq!(rest, strs(&["vorkath"]));
        assert!(bounds.after.is_some());
    }

    #[test]
    fn tokens_with_no_date_clause_are_left_entirely_alone() {
        let (bounds, rest) = extract_date_clauses(&strs(&["vorkath", ">1m"]), NOW(), EST_OFFSET);
        assert_eq!(bounds, DateBounds::default());
        assert_eq!(rest, strs(&["vorkath", ">1m"]));
    }

    #[test]
    fn plain_number_is_not_mistaken_for_a_last_n_clause_without_a_unit() {
        let (bounds, rest) = extract_date_clauses(&strs(&["last", "3", "kills"]), NOW(), EST_OFFSET);
        assert_eq!(bounds, DateBounds::default());
        assert_eq!(rest, strs(&["last", "3", "kills"]));
    }
}
