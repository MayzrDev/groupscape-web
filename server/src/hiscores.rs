//! Live OSRS hiscores lookup for a group member's boss/clue/minigame kill counts (the
//! `boss-kc-panel` frontend tab) - deliberately separate from GroupScape's own self-tracked
//! kill-count system (`leaderboard.rs`'s `LeaderboardMetric::BossKc`), which counts kills the
//! plugin itself observed. This module instead scrapes Jagex's own public hiscores every time
//! the tab is viewed, so it reflects the account's *real* lifetime totals (including kills from
//! before the player ever installed GroupScape).
//!
//! The `activities` array in Jagex's `index_lite.json` response is a fixed-order, fixed-name
//! list covering clue scrolls, minigames/leagues, and every boss - ground-truthed against a live
//! response for "Lynx Titan" on 2026-09-07 (91 entries, ids 0-90). [`ACTIVITY_DEFS`] mirrors that
//! exact order and exact `name` strings; a name Jagex adds later that isn't in this list is
//! logged and skipped rather than failing the whole request (see [`map_activities`]).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tokio::task;

const HISCORES_BASE: &str = "https://secure.runescape.com/m=hiscore_oldschool/index_lite.json";
const USER_AGENT: &str = "GroupScape/1.0 (contact: groupscape-admin@example.com)";

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HiscoreActivityCategory {
    Boss,
    Clue,
    Minigame,
}

/// One entry of Jagex's `activities` array, in their own order, resolved to a stable slug the
/// frontend uses for icon lookups. `kc` is the raw `score` field - `-1` means untracked/unranked,
/// preserved as-is rather than clamped to 0 so the frontend can render "--" for it.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HiscoreActivity {
    pub category: HiscoreActivityCategory,
    pub key: String,
    pub name: String,
    pub kc: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BossKcResponse {
    pub rsn: String,
    pub activities: Vec<HiscoreActivity>,
}

#[derive(Clone)]
pub enum HiscoresError {
    /// Jagex's hiscores endpoint 404d - either the account doesn't exist or the profile's
    /// hiscores are set to private.
    NotFound,
    /// Anything else - network failure, non-404 error status, unparseable body. Carries a
    /// message for logging only, never shown to the end user verbatim.
    Unavailable(String),
}

use HiscoreActivityCategory::{Boss, Clue, Minigame};

/// Ground-truthed via `curl -A "Mozilla/5.0" "https://secure.runescape.com/m=hiscore_oldschool/index_lite.json?player=Lynx%20Titan"`
/// on 2026-09-07 - see this module's doc comment. `(name, category, key)`.
const ACTIVITY_DEFS: &[(&str, HiscoreActivityCategory, &str)] = &[
    ("Grid Points", Minigame, "grid_points"),
    ("League Points", Minigame, "league_points"),
    ("Deadman Points", Minigame, "deadman_points"),
    ("Bounty Hunter - Hunter", Minigame, "bounty_hunter_hunter"),
    ("Bounty Hunter - Rogue", Minigame, "bounty_hunter_rogue"),
    ("Bounty Hunter (Legacy) - Hunter", Minigame, "bounty_hunter_legacy_hunter"),
    ("Bounty Hunter (Legacy) - Rogue", Minigame, "bounty_hunter_legacy_rogue"),
    ("Clue Scrolls (all)", Clue, "clue_all"),
    ("Clue Scrolls (beginner)", Clue, "clue_beginner"),
    ("Clue Scrolls (easy)", Clue, "clue_easy"),
    ("Clue Scrolls (medium)", Clue, "clue_medium"),
    ("Clue Scrolls (hard)", Clue, "clue_hard"),
    ("Clue Scrolls (elite)", Clue, "clue_elite"),
    ("Clue Scrolls (master)", Clue, "clue_master"),
    ("LMS - Rank", Minigame, "lms_rank"),
    ("PvP Arena - Rank", Minigame, "pvp_arena_rank"),
    ("Soul Wars Zeal", Minigame, "soul_wars_zeal"),
    ("Rifts closed", Minigame, "rifts_closed"),
    ("Colosseum Glory", Minigame, "colosseum_glory"),
    ("Collections Logged", Minigame, "collections_logged"),
    ("Abyssal Sire", Boss, "abyssal_sire"),
    ("Alchemical Hydra", Boss, "alchemical_hydra"),
    ("Amoxliatl", Boss, "amoxliatl"),
    ("Araxxor", Boss, "araxxor"),
    ("Artio", Boss, "artio"),
    ("Barrows Chests", Boss, "barrows_chests"),
    ("Bryophyta", Boss, "bryophyta"),
    ("Callisto", Boss, "callisto"),
    ("Calvar'ion", Boss, "calvarion"),
    ("Cerberus", Boss, "cerberus"),
    ("Chambers of Xeric", Boss, "chambers_of_xeric"),
    ("Chambers of Xeric: Challenge Mode", Boss, "chambers_of_xeric_cm"),
    ("Chaos Elemental", Boss, "chaos_elemental"),
    ("Chaos Fanatic", Boss, "chaos_fanatic"),
    ("Commander Zilyana", Boss, "commander_zilyana"),
    ("Corporeal Beast", Boss, "corporeal_beast"),
    ("Crazy Archaeologist", Boss, "crazy_archaeologist"),
    ("Dagannoth Prime", Boss, "dagannoth_prime"),
    ("Dagannoth Rex", Boss, "dagannoth_rex"),
    ("Dagannoth Supreme", Boss, "dagannoth_supreme"),
    ("Deranged Archaeologist", Boss, "deranged_archaeologist"),
    ("Doom of Mokhaiotl", Boss, "doom_of_mokhaiotl"),
    ("Duke Sucellus", Boss, "duke_sucellus"),
    ("General Graardor", Boss, "general_graardor"),
    ("Giant Mole", Boss, "giant_mole"),
    ("Grotesque Guardians", Boss, "grotesque_guardians"),
    ("Hespori", Boss, "hespori"),
    ("Kalphite Queen", Boss, "kalphite_queen"),
    ("King Black Dragon", Boss, "king_black_dragon"),
    ("Kraken", Boss, "kraken"),
    ("Kree'Arra", Boss, "kreearra"),
    ("K'ril Tsutsaroth", Boss, "kril_tsutsaroth"),
    ("Lunar Chests", Boss, "lunar_chests"),
    ("Mad Angel", Boss, "mad_angel"),
    ("Maggot King", Boss, "maggot_king"),
    ("Mimic", Boss, "mimic"),
    ("Nex", Boss, "nex"),
    ("Nightmare", Boss, "nightmare"),
    ("Phosani's Nightmare", Boss, "phosanis_nightmare"),
    ("Obor", Boss, "obor"),
    ("Phantom Muspah", Boss, "phantom_muspah"),
    ("Sarachnis", Boss, "sarachnis"),
    ("Scorpia", Boss, "scorpia"),
    ("Scurrius", Boss, "scurrius"),
    ("Shellbane Gryphon", Boss, "shellbane_gryphon"),
    ("Skotizo", Boss, "skotizo"),
    ("Sol Heredit", Boss, "sol_heredit"),
    ("Spindel", Boss, "spindel"),
    ("Tempoross", Boss, "tempoross"),
    ("The Gauntlet", Boss, "the_gauntlet"),
    ("The Corrupted Gauntlet", Boss, "the_corrupted_gauntlet"),
    ("The Hueycoatl", Boss, "the_hueycoatl"),
    ("The Leviathan", Boss, "the_leviathan"),
    ("The Royal Titans", Boss, "the_royal_titans"),
    ("The Whisperer", Boss, "the_whisperer"),
    ("Theatre of Blood", Boss, "theatre_of_blood"),
    ("Theatre of Blood: Hard Mode", Boss, "theatre_of_blood_hard_mode"),
    ("Thermonuclear Smoke Devil", Boss, "thermonuclear_smoke_devil"),
    ("Tombs of Amascut", Boss, "tombs_of_amascut"),
    ("Tombs of Amascut: Expert Mode", Boss, "tombs_of_amascut_expert_mode"),
    ("TzKal-Zuk", Boss, "tzkal_zuk"),
    ("TzTok-Jad", Boss, "tztok_jad"),
    ("Vardorvis", Boss, "vardorvis"),
    ("Venenatis", Boss, "venenatis"),
    ("Vet'ion", Boss, "vetion"),
    ("Vorkath", Boss, "vorkath"),
    ("Wintertodt", Boss, "wintertodt"),
    ("Yama", Boss, "yama"),
    ("Zalcano", Boss, "zalcano"),
    ("Zulrah", Boss, "zulrah"),
];

/// Every RSN's fetch shares this map so simultaneous viewers of the same member's panel (e.g.
/// two group members with the boss-kc tab open at once) trigger exactly one outbound request to
/// Jagex rather than one per viewer. Unlike `item_bonuses::IN_FLIGHT`, entries are *not* removed
/// as soon as they settle - they stay for [`DEDUPE_TTL`] after being inserted so a viewer whose
/// request lands just after another's already-finished fetch still gets the shared result,
/// keeping this data "live" (no long-lived cache) while still absorbing a request burst. A stale
/// entry is simply overwritten by the next request once its TTL has passed, starting a fresh
/// fetch.
struct InFlightEntry {
    inserted_at: Instant,
    cell: Arc<OnceCell<Result<BossKcResponse, HiscoresError>>>,
}
const DEDUPE_TTL: Duration = Duration::from_secs(10);

static IN_FLIGHT: LazyLock<Mutex<HashMap<String, InFlightEntry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns `rsn`'s live boss/clue/minigame kill counts from Jagex's hiscores, deduping
/// concurrent callers for the same RSN within a short window (see [`DEDUPE_TTL`]).
pub async fn get_boss_kc(rsn: &str) -> Result<BossKcResponse, HiscoresError> {
    let cell = {
        let mut in_flight = IN_FLIGHT.lock().unwrap();
        let now = Instant::now();
        let is_fresh = in_flight
            .get(rsn)
            .is_some_and(|entry| now.duration_since(entry.inserted_at) < DEDUPE_TTL);
        if !is_fresh {
            in_flight.insert(
                rsn.to_string(),
                InFlightEntry {
                    inserted_at: now,
                    cell: Arc::new(OnceCell::new()),
                },
            );
        }
        in_flight.get(rsn).unwrap().cell.clone()
    };

    let rsn_owned = rsn.to_string();
    cell.get_or_init(|| async move { fetch_boss_kc(rsn_owned).await })
        .await
        .clone()
}

async fn fetch_boss_kc(rsn: String) -> Result<BossKcResponse, HiscoresError> {
    task::spawn_blocking(move || fetch_boss_kc_sync(&rsn))
        .await
        .map_err(|err| HiscoresError::Unavailable(format!("fetch task panicked: {err}")))?
}

#[derive(Deserialize)]
struct RawHiscoresResponse {
    activities: Vec<RawActivity>,
}
#[derive(Deserialize)]
struct RawActivity {
    name: String,
    score: i64,
}

fn fetch_boss_kc_sync(rsn: &str) -> Result<BossKcResponse, HiscoresError> {
    let url = format!("{HISCORES_BASE}?player={}", urlencoding::encode(rsn));
    let mut response = match ureq::get(&url).header("User-Agent", USER_AGENT).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Err(HiscoresError::NotFound),
        Err(ureq::Error::StatusCode(code)) => {
            return Err(HiscoresError::Unavailable(format!("hiscores returned status {code}")))
        }
        Err(err) => return Err(HiscoresError::Unavailable(err.to_string())),
    };

    let raw: RawHiscoresResponse = response
        .body_mut()
        .read_json()
        .map_err(|err| HiscoresError::Unavailable(format!("failed to parse hiscores response: {err}")))?;

    Ok(BossKcResponse {
        rsn: rsn.to_string(),
        activities: map_activities(raw.activities),
    })
}

/// Resolves Jagex's raw `activities` entries to [`ACTIVITY_DEFS`] by exact name match, in
/// [`ACTIVITY_DEFS`]'s order (== Jagex's own order, ground-truthed - see this module's doc
/// comment) rather than the raw response's order, so a future Jagex reordering can't silently
/// reorder the panel. Any raw entry whose name isn't in [`ACTIVITY_DEFS`] (a brand new hiscores
/// category added since the last ground-truthing pass) is logged and skipped rather than failing
/// the whole request.
fn map_activities(raw: Vec<RawActivity>) -> Vec<HiscoreActivity> {
    let by_name: HashMap<&str, i64> = raw.iter().map(|a| (a.name.as_str(), a.score)).collect();

    let mut seen_names: HashSet<&str> = HashSet::new();
    let mut activities = Vec::with_capacity(ACTIVITY_DEFS.len());
    for (name, category, key) in ACTIVITY_DEFS {
        seen_names.insert(name);
        let kc = by_name.get(name).copied().unwrap_or(-1);
        activities.push(HiscoreActivity {
            category: *category,
            key: (*key).to_string(),
            name: (*name).to_string(),
            kc,
        });
    }

    for raw_activity in &raw {
        if !seen_names.contains(raw_activity.name.as_str()) {
            log::warn!(
                "hiscores activity \"{}\" is not in ACTIVITY_DEFS - skipping (may be a new Jagex category)",
                raw_activity.name
            );
        }
    }

    activities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_activities_in_def_order_regardless_of_raw_order() {
        let raw = vec![
            RawActivity { name: "Zulrah".to_string(), score: 42 },
            RawActivity { name: "Grid Points".to_string(), score: -1 },
            RawActivity { name: "Clue Scrolls (all)".to_string(), score: 7 },
        ];
        let activities = map_activities(raw);
        assert_eq!(activities.len(), ACTIVITY_DEFS.len());
        assert_eq!(activities[0].name, "Grid Points");
        assert_eq!(activities[0].kc, -1);
        let clue_all = activities.iter().find(|a| a.key == "clue_all").unwrap();
        assert_eq!(clue_all.kc, 7);
        assert!(matches!(clue_all.category, HiscoreActivityCategory::Clue));
        let zulrah = activities.iter().find(|a| a.key == "zulrah").unwrap();
        assert_eq!(zulrah.kc, 42);
        assert!(matches!(zulrah.category, HiscoreActivityCategory::Boss));
    }

    #[test]
    fn untracked_activity_defaults_to_negative_one() {
        let activities = map_activities(vec![]);
        assert!(activities.iter().all(|a| a.kc == -1));
    }

    #[test]
    fn unknown_raw_activity_is_skipped_without_error() {
        let raw = vec![RawActivity {
            name: "Some Brand New Boss".to_string(),
            score: 3,
        }];
        let activities = map_activities(raw);
        assert_eq!(activities.len(), ACTIVITY_DEFS.len());
    }

    #[test]
    fn activity_defs_keys_are_unique() {
        let mut keys: Vec<&str> = ACTIVITY_DEFS.iter().map(|(_, _, key)| *key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ACTIVITY_DEFS.len());
    }
}
