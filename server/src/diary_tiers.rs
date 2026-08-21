//! Achievement-diary bit layout, transliterated verbatim from `parseDiaryData` in
//! `site/src/data/diaries.js`.
//!
//! NOTE: this is a manual-sync duplicate of the site's copy - the bit layout lives nowhere else
//! (there's no data file for it), and the server needs it to detect newly-completed diary tiers
//! from a raw `diary_vars` upload. Same manual-sync tradeoff already accepted by
//! [`crate::quest_ids`] / [`crate::notable_npcs`]. Keep the var indexes, bit indexes, and the
//! `values()`-style regions (Karamja) in sync if the site's copy changes.

/// One region+tier's per-task completion flags, in the same order the site renders them.
pub struct DiaryTier {
    pub region: &'static str,
    pub tier: &'static str,
    pub entries: Vec<bool>,
}

impl DiaryTier {
    pub fn is_complete(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|entry| *entry)
    }
}

/// Mirrors `utility.isBitSet` (JS 32-bit bitwise semantics), reading past the end of a short
/// `diary_vars` upload as an unset bit rather than panicking.
fn is_bit_set(diary_vars: &[i32], var_index: usize, bit_index: u32) -> bool {
    diary_vars
        .get(var_index)
        .is_some_and(|value| ((*value as u32) >> bit_index) & 1 == 1)
}

fn bits(diary_vars: &[i32], var_index: usize, bit_indexes: &[u32]) -> Vec<bool> {
    bit_indexes
        .iter()
        .map(|bit_index| is_bit_set(diary_vars, var_index, *bit_index))
        .collect()
}

fn values(diary_vars: &[i32], checks: &[(usize, i32)]) -> Vec<bool> {
    checks
        .iter()
        .map(|(var_index, expected)| diary_vars.get(*var_index) == Some(expected))
        .collect()
}

fn concat(parts: Vec<Vec<bool>>) -> Vec<bool> {
    parts.into_iter().flatten().collect()
}

/// Computes the same `{ region: { tier: [bool] } }` structure the client computes, flattened to a
/// list so callers can diff it directly.
pub fn parse_diary_data(diary_vars: &[i32]) -> Vec<DiaryTier> {
    let v = diary_vars;
    let mut result = Vec::new();
    let mut push = |region: &'static str, tier: &'static str, entries: Vec<bool>| {
        result.push(DiaryTier {
            region,
            tier,
            entries,
        });
    };

    push(
        "Ardougne",
        "Easy",
        bits(v, 0, &[0, 1, 2, 4, 5, 6, 7, 9, 11, 12]),
    );
    push(
        "Ardougne",
        "Medium",
        bits(v, 0, &[13, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25]),
    );
    push(
        "Ardougne",
        "Hard",
        concat(vec![
            bits(v, 0, &[26, 27, 28, 29, 30, 31]),
            bits(v, 1, &[0, 1, 2, 3, 4, 5]),
        ]),
    );
    push(
        "Ardougne",
        "Elite",
        bits(v, 1, &[6, 7, 9, 8, 10, 11, 12, 13]),
    );

    push(
        "Desert",
        "Easy",
        bits(v, 2, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    );
    push(
        "Desert",
        "Medium",
        concat(vec![
            bits(v, 2, &[12, 13, 14, 15, 16, 17, 18, 19, 20, 21]),
            vec![is_bit_set(v, 2, 22) || is_bit_set(v, 3, 9)],
            bits(v, 2, &[23]),
        ]),
    );
    push(
        "Desert",
        "Hard",
        concat(vec![
            bits(v, 2, &[24, 25, 26, 27, 28, 29, 30, 31]),
            bits(v, 3, &[0, 1]),
        ]),
    );
    push("Desert", "Elite", bits(v, 3, &[2, 4, 5, 6, 7, 8]));

    push(
        "Falador",
        "Easy",
        bits(v, 4, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
    );
    push(
        "Falador",
        "Medium",
        bits(
            v,
            4,
            &[11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 22, 23, 24, 25],
        ),
    );
    push(
        "Falador",
        "Hard",
        concat(vec![
            bits(v, 4, &[26, 27, 28, 29, 30, 31]),
            bits(v, 5, &[0, 1, 2, 3, 4]),
        ]),
    );
    push("Falador", "Elite", bits(v, 5, &[5, 6, 7, 8, 9, 10]));

    push(
        "Fremennik",
        "Easy",
        bits(v, 6, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
    );
    push(
        "Fremennik",
        "Medium",
        bits(v, 6, &[11, 12, 13, 14, 15, 17, 18, 19, 20]),
    );
    push(
        "Fremennik",
        "Hard",
        bits(v, 6, &[21, 23, 24, 25, 26, 27, 28, 29, 30]),
    );
    push(
        "Fremennik",
        "Elite",
        concat(vec![bits(v, 6, &[31]), bits(v, 7, &[0, 1, 2, 3, 4])]),
    );

    push(
        "Kandarin",
        "Easy",
        bits(v, 8, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    );
    push(
        "Kandarin",
        "Medium",
        bits(
            v,
            8,
            &[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25],
        ),
    );
    push(
        "Kandarin",
        "Hard",
        concat(vec![
            bits(v, 8, &[26, 27, 28, 29, 30, 31]),
            bits(v, 9, &[0, 1, 2, 3, 4]),
        ]),
    );
    push("Kandarin", "Elite", bits(v, 9, &[5, 6, 7, 8, 9, 10, 11]));

    push(
        "Karamja",
        "Easy",
        values(
            v,
            &[
                (23, 5),
                (24, 1),
                (25, 1),
                (26, 1),
                (27, 1),
                (28, 1),
                (29, 1),
                (30, 5),
                (31, 1),
                (32, 1),
            ],
        ),
    );
    push(
        "Karamja",
        "Medium",
        values(
            v,
            &[
                (33, 1),
                (34, 1),
                (35, 1),
                (36, 1),
                (37, 1),
                (38, 1),
                (39, 1),
                (40, 1),
                (41, 1),
                (42, 1),
                (43, 1),
                (44, 1),
                (45, 1),
                (46, 1),
                (47, 1),
                (48, 1),
                (49, 1),
                (50, 1),
                (51, 1),
            ],
        ),
    );
    push(
        "Karamja",
        "Hard",
        values(
            v,
            &[
                (52, 1),
                (53, 1),
                (54, 1),
                (55, 1),
                (56, 1),
                (57, 1),
                (58, 1),
                (59, 5),
                (60, 1),
                (61, 1),
            ],
        ),
    );
    push("Karamja", "Elite", bits(v, 10, &[1, 2, 3, 4, 5]));

    push(
        "Kourend & Kebos",
        "Easy",
        bits(v, 11, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
    );
    push(
        "Kourend & Kebos",
        "Medium",
        bits(v, 11, &[25, 13, 14, 15, 21, 16, 17, 18, 19, 22, 20, 23, 24]),
    );
    push(
        "Kourend & Kebos",
        "Hard",
        concat(vec![
            bits(v, 11, &[26, 27, 28, 29, 31, 30]),
            bits(v, 12, &[0, 1, 2, 3]),
        ]),
    );
    push(
        "Kourend & Kebos",
        "Elite",
        bits(v, 12, &[4, 5, 6, 7, 8, 9, 10, 11]),
    );

    push(
        "Lumbridge & Draynor",
        "Easy",
        bits(v, 13, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
    );
    push(
        "Lumbridge & Draynor",
        "Medium",
        bits(v, 13, &[13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]),
    );
    push(
        "Lumbridge & Draynor",
        "Hard",
        concat(vec![
            bits(v, 13, &[25, 26, 27, 28, 29, 30, 31]),
            bits(v, 14, &[0, 1, 2, 3]),
        ]),
    );
    push(
        "Lumbridge & Draynor",
        "Elite",
        bits(v, 14, &[4, 5, 6, 7, 8, 9]),
    );

    push(
        "Morytania",
        "Easy",
        bits(v, 15, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    );
    push(
        "Morytania",
        "Medium",
        bits(v, 15, &[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22]),
    );
    push(
        "Morytania",
        "Hard",
        concat(vec![
            bits(v, 15, &[23, 24, 25, 26, 27, 28, 29, 30]),
            bits(v, 16, &[1, 2]),
        ]),
    );
    push("Morytania", "Elite", bits(v, 16, &[3, 4, 5, 6, 7, 8]));

    push(
        "Varrock",
        "Easy",
        bits(v, 17, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]),
    );
    push(
        "Varrock",
        "Medium",
        bits(v, 17, &[15, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]),
    );
    push(
        "Varrock",
        "Hard",
        concat(vec![
            bits(v, 17, &[29, 30, 31]),
            bits(v, 18, &[0, 1, 2, 3, 4, 5, 6]),
        ]),
    );
    push("Varrock", "Elite", bits(v, 18, &[7, 8, 9, 10, 11]));

    push(
        "Western Provinces",
        "Easy",
        bits(v, 19, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    );
    push(
        "Western Provinces",
        "Medium",
        bits(v, 19, &[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]),
    );
    push(
        "Western Provinces",
        "Hard",
        concat(vec![
            bits(v, 19, &[25, 26, 27, 28, 29, 30, 31]),
            bits(v, 20, &[0, 1, 2, 3, 4, 5]),
        ]),
    );
    push(
        "Western Provinces",
        "Elite",
        bits(v, 20, &[6, 7, 8, 9, 12, 13, 14]),
    );

    push(
        "Wilderness",
        "Easy",
        bits(v, 21, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
    );
    push(
        "Wilderness",
        "Medium",
        bits(v, 21, &[13, 14, 15, 16, 18, 19, 20, 21, 22, 23, 24]),
    );
    push(
        "Wilderness",
        "Hard",
        concat(vec![
            bits(v, 21, &[25, 26, 27, 28, 29, 30, 31]),
            bits(v, 22, &[0, 1, 2]),
        ]),
    );
    push("Wilderness", "Elite", bits(v, 22, &[3, 5, 7, 8, 9, 10, 11]));

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ported from `site/test/diaries.test.js`'s `allCompleteDiaryVars` fixture.
    fn all_complete_diary_vars() -> Vec<i32> {
        let mut vars = vec![0xffffffffu32 as i32; 62];
        vars[23] = 5;
        vars[24..=29].fill(1);
        vars[30] = 5;
        vars[31] = 1;
        vars[32] = 1;
        vars[33..=58].fill(1);
        vars[59] = 5;
        vars[60] = 1;
        vars[61] = 1;
        vars
    }

    #[test]
    fn maps_every_diary_entry_to_a_boolean_completion_value() {
        let empty = parse_diary_data(&vec![0; 62]);
        let complete = parse_diary_data(&all_complete_diary_vars());
        assert_eq!(empty.len(), complete.len());
        // 12 diary regions x 4 tiers, matching `parseDiaryData`'s structure.
        assert_eq!(complete.len(), 12 * 4);

        for (empty_tier, complete_tier) in empty.iter().zip(complete.iter()) {
            assert!(!complete_tier.entries.is_empty());
            assert!(empty_tier.entries.iter().all(|entry| !*entry));
            assert!(
                complete_tier.entries.iter().all(|entry| *entry),
                "{} {} should be fully complete",
                complete_tier.region,
                complete_tier.tier
            );
            assert!(complete_tier.is_complete());
            assert!(!empty_tier.is_complete());
        }
    }

    #[test]
    fn supports_desert_medium_completion_via_the_alternate_bit() {
        let mut vars = vec![0i32; 62];
        vars[3] = 1 << 9;

        let parsed = parse_diary_data(&vars);
        let desert_medium = parsed
            .iter()
            .find(|tier| tier.region == "Desert" && tier.tier == "Medium")
            .expect("Desert Medium must exist");

        assert!(desert_medium.entries[10]);
    }

    #[test]
    fn short_diary_vars_upload_reads_as_incomplete_rather_than_panicking() {
        let parsed = parse_diary_data(&[]);
        assert!(parsed.iter().all(|tier| !tier.is_complete()));
    }
}
