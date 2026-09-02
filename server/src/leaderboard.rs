use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardMetric {
    Xp,
    BossKc,
    GpEarned,
    RaidCompletions,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaderboardWindow {
    Daily,
    Weekly,
    AllTime,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct LeaderboardEntry {
    pub member_name: String,
    pub value: i64,
    pub rank: i64,
}

#[derive(Serialize, Deserialize)]
pub struct LeaderboardResult {
    pub metric: LeaderboardMetric,
    pub window: LeaderboardWindow,
    pub boss: Option<String>,
    pub available_bosses: Vec<String>,
    pub entries: Vec<LeaderboardEntry>,
}

impl Serialize for LeaderboardMetric {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            LeaderboardMetric::Xp => "xp",
            LeaderboardMetric::BossKc => "boss_kc",
            LeaderboardMetric::GpEarned => "gp_earned",
            LeaderboardMetric::RaidCompletions => "raid_completions",
        };
        serializer.serialize_str(s)
    }
}

impl Serialize for LeaderboardWindow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            LeaderboardWindow::Daily => "daily",
            LeaderboardWindow::Weekly => "weekly",
            LeaderboardWindow::AllTime => "all_time",
        };
        serializer.serialize_str(s)
    }
}

/// Standard competition ranking (1, 1, 3 - not 1, 1, 2): tied values share a rank and the
/// next distinct value skips ahead by the tie group's size. Ties break alphabetically by
/// member name for a stable, deterministic order. Ported from `groupscape-old`'s
/// `rankEntries` (`apps/api/src/lib/leaderboardMetrics.ts`).
pub fn rank_entries(entries: Vec<(String, i64)>) -> Vec<LeaderboardEntry> {
    let mut sorted = entries;
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut ranked = Vec::with_capacity(sorted.len());
    let mut rank = 0i64;
    let mut last_value: Option<i64> = None;
    for (index, (member_name, value)) in sorted.into_iter().enumerate() {
        if last_value != Some(value) {
            rank = index as i64 + 1;
            last_value = Some(value);
        }
        ranked.push(LeaderboardEntry {
            member_name,
            value,
            rank,
        });
    }
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(pairs: &[(&str, i64)]) -> Vec<(String, i64)> {
        pairs.iter().map(|(n, v)| (n.to_string(), *v)).collect()
    }

    #[test]
    fn ranks_distinct_values_in_descending_order() {
        let ranked = rank_entries(entries(&[("a", 10), ("b", 30), ("c", 20)]));
        assert_eq!(
            ranked,
            vec![
                LeaderboardEntry { member_name: "b".into(), value: 30, rank: 1 },
                LeaderboardEntry { member_name: "c".into(), value: 20, rank: 2 },
                LeaderboardEntry { member_name: "a".into(), value: 10, rank: 3 },
            ]
        );
    }

    #[test]
    fn shares_rank_across_ties_and_skips_the_next_rank() {
        let ranked = rank_entries(entries(&[("a", 10), ("b", 10), ("c", 5)]));
        assert_eq!(
            ranked,
            vec![
                LeaderboardEntry { member_name: "a".into(), value: 10, rank: 1 },
                LeaderboardEntry { member_name: "b".into(), value: 10, rank: 1 },
                LeaderboardEntry { member_name: "c".into(), value: 5, rank: 3 },
            ]
        );
    }

    #[test]
    fn breaks_ties_alphabetically_by_member_name() {
        let ranked = rank_entries(entries(&[("zed", 10), ("amy", 10)]));
        assert_eq!(ranked[0].member_name, "amy");
        assert_eq!(ranked[1].member_name, "zed");
        assert_eq!(ranked[0].rank, 1);
        assert_eq!(ranked[1].rank, 1);
    }

    #[test]
    fn empty_input_ranks_to_empty_output() {
        assert!(rank_entries(vec![]).is_empty());
    }
}
