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
}
