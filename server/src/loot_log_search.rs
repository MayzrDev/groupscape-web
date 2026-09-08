use std::collections::HashMap;
use std::sync::LazyLock;

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
}
