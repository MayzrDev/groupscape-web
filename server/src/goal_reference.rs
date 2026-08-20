use crate::drop_rates::slugify_npc_name;

/// §20/§22 (`groupscape-old`) combat achievement tiers. Kept as this module's own list rather
/// than reusing anything from `notable_npcs`/`drop_rates` since it's an OSRS-fixed enum, not
/// content-catalog data.
pub const CA_TIERS: &[&str] = &["easy", "medium", "hard", "elite", "master", "grandmaster"];

/// Duplicated 23-skill list - same "small curated content list, no server catalog to import
/// from" pattern `groupscape-old` used (`apps/web/src/leaderboards/skillsList.ts`).
pub const SKILLS: &[&str] = &[
    "attack",
    "defence",
    "strength",
    "hitpoints",
    "ranged",
    "prayer",
    "magic",
    "cooking",
    "woodcutting",
    "fletching",
    "fishing",
    "firemaking",
    "crafting",
    "smithing",
    "mining",
    "herblore",
    "agility",
    "thieving",
    "slayer",
    "farming",
    "runecraft",
    "hunter",
    "construction",
];

pub struct GoalReferenceValidation {
    pub reference_id: Option<String>,
    pub error: Option<&'static str>,
}

/// Ported from `groupscape-old`'s `validateGoalReference` (`apps/api/src/lib/goalReference.ts`).
/// No item/quest catalog exists in this codebase yet, so `item`/`quest` reference_ids are
/// accepted as free text rather than validated against a real catalog - an honest gap in the
/// same spirit as this codebase's other "representative, not exhaustive" content files.
/// `skill`/`ca_tier` do have a real fixed enum, so those are validated for real; `boss` is
/// normalized through the same `slugify_npc_name` the drop-rates lookup uses, so a goal's
/// reference_id lines up with `activity_events` kill payloads for future auto-complete wiring.
pub fn validate_goal_reference(
    reference_type: &str,
    raw_reference_id: Option<&str>,
) -> GoalReferenceValidation {
    if reference_type == "none" {
        return GoalReferenceValidation {
            reference_id: None,
            error: None,
        };
    }

    let trimmed = raw_reference_id.map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return GoalReferenceValidation {
            reference_id: None,
            error: Some("reference_id is required for this reference_type"),
        };
    }

    match reference_type {
        "skill" => {
            let normalized = trimmed.to_ascii_lowercase();
            if SKILLS.contains(&normalized.as_str()) {
                GoalReferenceValidation {
                    reference_id: Some(normalized),
                    error: None,
                }
            } else {
                GoalReferenceValidation {
                    reference_id: None,
                    error: Some("Unknown skill"),
                }
            }
        }
        "ca_tier" => {
            let normalized = trimmed.to_ascii_lowercase();
            if CA_TIERS.contains(&normalized.as_str()) {
                GoalReferenceValidation {
                    reference_id: Some(normalized),
                    error: None,
                }
            } else {
                GoalReferenceValidation {
                    reference_id: None,
                    error: Some("Unknown combat achievement tier"),
                }
            }
        }
        "boss" => GoalReferenceValidation {
            reference_id: Some(slugify_npc_name(trimmed)),
            error: None,
        },
        "item" | "quest" => GoalReferenceValidation {
            reference_id: Some(trimmed.to_string()),
            error: None,
        },
        _ => GoalReferenceValidation {
            reference_id: None,
            error: Some("Unknown reference_type"),
        },
    }
}

/// All `reference_type` values this goal schema accepts - mirrors `groupscape-old`'s
/// `GOAL_REFERENCE_TYPES` (`repositories/groupGoals.ts`).
pub const GOAL_REFERENCE_TYPES: &[&str] = &["item", "boss", "quest", "skill", "ca_tier", "none"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_type_requires_no_reference_id() {
        let result = validate_goal_reference("none", None);
        assert!(result.error.is_none());
        assert_eq!(result.reference_id, None);
    }

    #[test]
    fn skill_normalizes_and_validates_against_the_fixed_list() {
        let result = validate_goal_reference("skill", Some("Slayer"));
        assert!(result.error.is_none());
        assert_eq!(result.reference_id.as_deref(), Some("slayer"));

        let bad = validate_goal_reference("skill", Some("not_a_skill"));
        assert_eq!(bad.error, Some("Unknown skill"));
    }

    #[test]
    fn ca_tier_normalizes_and_validates_against_the_fixed_list() {
        let result = validate_goal_reference("ca_tier", Some("Grandmaster"));
        assert!(result.error.is_none());
        assert_eq!(result.reference_id.as_deref(), Some("grandmaster"));

        let bad = validate_goal_reference("ca_tier", Some("legendary"));
        assert_eq!(bad.error, Some("Unknown combat achievement tier"));
    }

    #[test]
    fn boss_reference_id_is_slugified() {
        let result = validate_goal_reference("boss", Some("K'ril Tsutsaroth"));
        assert!(result.error.is_none());
        assert_eq!(result.reference_id.as_deref(), Some("kril_tsutsaroth"));
    }

    #[test]
    fn item_and_quest_pass_through_as_free_text() {
        let item = validate_goal_reference("item", Some("Twisted bow"));
        assert_eq!(item.reference_id.as_deref(), Some("Twisted bow"));

        let quest = validate_goal_reference("quest", Some("Monkey Madness II"));
        assert_eq!(quest.reference_id.as_deref(), Some("Monkey Madness II"));
    }

    #[test]
    fn non_none_type_requires_a_non_empty_reference_id() {
        let result = validate_goal_reference("skill", Some("   "));
        assert_eq!(
            result.error,
            Some("reference_id is required for this reference_type")
        );
    }

    #[test]
    fn unknown_reference_type_is_rejected() {
        let result = validate_goal_reference("not_a_type", Some("x"));
        assert_eq!(result.error, Some("Unknown reference_type"));
    }
}
