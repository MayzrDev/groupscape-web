use crate::db;
use crate::error::ApiError;
use crate::models::{CombatStyleBonuses, ItemBonusesResponse};
use deadpool_postgres::Pool;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::OnceCell;
use tokio::task;
use ureq::ResponseExt;

const WIKI_BASE: &str = "https://oldschool.runescape.wiki";
const USER_AGENT: &str = "GroupScape/1.0 (contact: groupscape-admin@example.com)";

/// Every equipped item's bonuses fetch shares this map so simultaneous viewers of the same item
/// (e.g. a common weapon in a group) trigger exactly one wiki scrape rather than one per
/// requester. Entries are removed once their [`OnceCell`] settles, so a later cache-expiry
/// re-scrape starts a fresh dedupe cycle instead of replaying a stale result forever.
static IN_FLIGHT: LazyLock<Mutex<HashMap<i32, Arc<OnceCell<Result<ItemBonusesResponse, String>>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns `item_id`'s equipment bonuses, from the 30-day Postgres cache
/// (`groupscape.item_bonuses`) when fresh, otherwise scraping the OSRS Wiki and caching the
/// result. Concurrent callers for the same `item_id` share a single in-flight scrape.
pub async fn get_item_bonuses(db_pool: &Pool, item_id: i32) -> Result<ItemBonusesResponse, ApiError> {
    {
        let client = db_pool.get().await.map_err(ApiError::PoolError)?;
        if let Some(cached) = db::get_cached_item_bonuses(&client, item_id).await? {
            return Ok(cached);
        }
    }

    let cell = {
        let mut in_flight = IN_FLIGHT.lock().unwrap();
        in_flight
            .entry(item_id)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    };

    let pool = db_pool.clone();
    let result = cell
        .get_or_init(|| async move { fetch_and_cache(pool, item_id).await.map_err(|err| err.to_string()) })
        .await
        .clone();

    IN_FLIGHT.lock().unwrap().remove(&item_id);

    result.map_err(ApiError::ItemBonusesScrapeError)
}

async fn fetch_and_cache(db_pool: Pool, item_id: i32) -> Result<ItemBonusesResponse, ApiError> {
    let bonuses = scrape_item_bonuses(item_id).await?;
    let response = match bonuses {
        Some(raw) => raw.into_response(item_id),
        // Not equippable / no wiki page found - still cached, so a bank/inventory item that
        // gets viewed repeatedly doesn't trigger a wiki round-trip every time.
        None => zero_response(item_id),
    };

    let client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::upsert_item_bonuses(&client, &response).await?;
    Ok(response)
}

fn zero_response(item_id: i32) -> ItemBonusesResponse {
    let zero_style = CombatStyleBonuses {
        stab: 0,
        slash: 0,
        crush: 0,
        magic: 0,
        ranged: 0,
    };
    ItemBonusesResponse {
        item_id,
        attack: zero_style.clone(),
        defence: zero_style,
        melee_strength: 0,
        ranged_strength: 0,
        magic_damage: 0,
        prayer: 0,
        attack_speed: None,
    }
}

struct RawBonuses {
    attack_stab: i32,
    attack_slash: i32,
    attack_crush: i32,
    attack_magic: i32,
    attack_ranged: i32,
    defence_stab: i32,
    defence_slash: i32,
    defence_crush: i32,
    defence_magic: i32,
    defence_ranged: i32,
    melee_strength: i32,
    ranged_strength: i32,
    magic_damage: i32,
    prayer: i32,
    attack_speed: Option<i32>,
}
impl RawBonuses {
    fn into_response(self, item_id: i32) -> ItemBonusesResponse {
        ItemBonusesResponse {
            item_id,
            attack: CombatStyleBonuses {
                stab: self.attack_stab,
                slash: self.attack_slash,
                crush: self.attack_crush,
                magic: self.attack_magic,
                ranged: self.attack_ranged,
            },
            defence: CombatStyleBonuses {
                stab: self.defence_stab,
                slash: self.defence_slash,
                crush: self.defence_crush,
                magic: self.defence_magic,
                ranged: self.defence_ranged,
            },
            melee_strength: self.melee_strength,
            ranged_strength: self.ranged_strength,
            magic_damage: self.magic_damage,
            prayer: self.prayer,
            attack_speed: self.attack_speed,
        }
    }
}

/// Runs the (blocking) scrape on a worker thread, mirroring `discord.rs`'s
/// `task::spawn_blocking` pattern for `ureq` calls - there is no async HTTP client in this
/// codebase.
async fn scrape_item_bonuses(item_id: i32) -> Result<Option<RawBonuses>, ApiError> {
    task::spawn_blocking(move || scrape_item_bonuses_sync(item_id))
        .await
        .map_err(|err| ApiError::ItemBonusesScrapeError(format!("scrape task panicked: {err}")))?
}

fn scrape_item_bonuses_sync(item_id: i32) -> Result<Option<RawBonuses>, ApiError> {
    let Some(title) = resolve_page_title(item_id)? else {
        return Ok(None);
    };
    let Some(wikitext) = fetch_wikitext(&title)? else {
        return Ok(None);
    };

    let blocks = extract_template_blocks(&wikitext, "infobox bonuses");
    if blocks.is_empty() {
        return Ok(None);
    }

    let mut best: Option<(HashMap<String, String>, Option<usize>)> = None;
    for block in &blocks {
        let params = parse_template_params(block);
        let version_index = find_version_index(&params, item_id);
        let matches_base_id = params
            .get("id")
            .and_then(|v| v.trim().parse::<i32>().ok())
            == Some(item_id);
        if version_index.is_some() || matches_base_id {
            best = Some((params, version_index));
            break;
        }
        if best.is_none() {
            best = Some((params, None));
        }
    }
    let Some((params, version_index)) = best else {
        return Ok(None);
    };

    Ok(Some(bonuses_from_params(&params, version_index)))
}

/// Resolves `item_id` to its wiki page via the id-lookup redirect, rather than matching on item
/// name - names collide across unrelated items and don't handle disambiguation the way the
/// wiki's own id index does. Returns `None` if the id has no lookup entry (untradeable/removed
/// items, or ids outside the wiki's index).
fn resolve_page_title(item_id: i32) -> Result<Option<String>, ApiError> {
    let url = format!("{WIKI_BASE}/w/Special:Lookup?type=item&id={item_id}");
    let response = match ureq::get(&url).header("User-Agent", USER_AGENT).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(_)) => return Ok(None),
        Err(err) => return Err(ApiError::UreqError(err)),
    };

    let path = response.get_uri().path().to_string();
    let Some(title_enc) = path.strip_prefix("/w/") else {
        return Ok(None);
    };
    let decoded = urlencoding::decode(title_enc)
        .map_err(|err| ApiError::ItemBonusesScrapeError(format!("bad wiki title encoding: {err}")))?;
    Ok(Some(decoded.replace('_', " ")))
}

#[derive(Deserialize)]
struct ParseApiResponse {
    parse: Option<ParseResult>,
}
#[derive(Deserialize)]
struct ParseResult {
    wikitext: Option<WikitextField>,
}
#[derive(Deserialize)]
struct WikitextField {
    #[serde(rename = "*")]
    content: String,
}

fn fetch_wikitext(title: &str) -> Result<Option<String>, ApiError> {
    let url = format!(
        "{WIKI_BASE}/api.php?action=parse&page={}&prop=wikitext&format=json",
        urlencoding::encode(title)
    );
    let mut response = match ureq::get(&url).header("User-Agent", USER_AGENT).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(_)) => return Ok(None),
        Err(err) => return Err(ApiError::UreqError(err)),
    };
    let body: ParseApiResponse = response.body_mut().read_json().map_err(ApiError::UreqError)?;
    Ok(body.parse.and_then(|p| p.wikitext).map(|w| w.content))
}

/// Finds every top-level `{{<name> ... }}` occurrence in `wikitext`, brace-depth matched so
/// templates nested inside (e.g. `{{plink|...}}` in an examine line) don't truncate the match.
fn extract_template_blocks(wikitext: &str, name: &str) -> Vec<String> {
    let chars: Vec<char> = wikitext.chars().collect();
    let lower: Vec<char> = wikitext.to_lowercase().chars().collect();
    let marker: Vec<char> = format!("{{{{{}", name.to_lowercase()).chars().collect();

    let mut blocks = Vec::new();
    let mut search_from = 0usize;

    'outer: loop {
        let Some(start) = find_subslice(&lower, &marker, search_from) else {
            break;
        };

        let after = chars.get(start + marker.len());
        if let Some(&after_char) = after {
            if !(after_char.is_whitespace() || after_char == '|' || after_char == '}') {
                search_from = start + marker.len();
                continue 'outer;
            }
        }

        let mut depth = 0i32;
        let mut end: Option<usize> = None;
        let mut i = start;
        while i + 1 < chars.len() {
            let two = (chars[i], chars[i + 1]);
            if two == ('{', '{') {
                depth += 1;
                i += 2;
                continue;
            }
            if two == ('}', '}') {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
                continue;
            }
            i += 1;
        }
        let Some(end) = end else { break };

        blocks.push(chars[start + 2..end - 2].iter().collect());
        search_from = end;
    }

    blocks
}

fn find_subslice(haystack: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}

/// Splits a `{{Template|a=1|b=[[File:x.png|100px]]}}` body into `{a: "1", b: "[[File:x.png|100px]]"}`,
/// treating `|` as a delimiter only at bracket/brace depth 0 so wikilinks/nested templates inside
/// a param value (e.g. `|image = [[File:...|150px]]`) don't get mis-split.
fn parse_template_params(body: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let chars: Vec<char> = body.chars().collect();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut parts = Vec::new();

    let mut i = 0;
    while i < chars.len() {
        let two = if i + 1 < chars.len() {
            Some((chars[i], chars[i + 1]))
        } else {
            None
        };
        match two {
            Some(('[', '[')) | Some(('{', '{')) => {
                depth += 1;
                current.push(chars[i]);
                current.push(chars[i + 1]);
                i += 2;
                continue;
            }
            Some((']', ']')) | Some(('}', '}')) => {
                depth -= 1;
                current.push(chars[i]);
                current.push(chars[i + 1]);
                i += 2;
                continue;
            }
            _ => {}
        }
        if chars[i] == '|' && depth == 0 {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(chars[i]);
        }
        i += 1;
    }
    parts.push(current);

    for part in parts {
        if let Some(eq) = part.find('=') {
            let key = part[..eq].trim().to_lowercase();
            let value = part[eq + 1..].trim().to_string();
            if !key.is_empty() {
                params.insert(key, value);
            }
        }
    }
    params
}

/// Multi-version items (degradable/charged/broken variants sharing one wiki page) list each
/// version's stats either as numbered-suffix params (`astab1`, `astab2`, paired with `id1`,
/// `id2`) or as one comma-separated value per param (`id = 1234, 5678`) - this figures out
/// which positional slot `item_id` occupies, if any. Single-version pages (the common case)
/// have no such suffixing and this returns `None`, meaning "use params as-is".
fn find_version_index(params: &HashMap<String, String>, item_id: i32) -> Option<usize> {
    if let Some(id_field) = params.get("id") {
        let parts: Vec<i32> = id_field
            .split(',')
            .filter_map(|s| s.trim().parse::<i32>().ok())
            .collect();
        if parts.len() > 1 {
            if let Some(idx) = parts.iter().position(|&id| id == item_id) {
                return Some(idx);
            }
        }
    }
    for i in 1..=20 {
        if let Some(v) = params.get(&format!("id{i}")) {
            if v.trim().parse::<i32>().ok() == Some(item_id) {
                return Some(i - 1);
            }
        }
    }
    None
}

fn get_param<'a>(
    params: &'a HashMap<String, String>,
    key: &str,
    version_index: Option<usize>,
) -> Option<&'a str> {
    let Some(version_index) = version_index else {
        return params.get(key).map(String::as_str);
    };
    if let Some(suffixed) = params.get(&format!("{key}{}", version_index + 1)) {
        return Some(suffixed.as_str());
    }
    let base = params.get(key)?;
    if base.contains(',') {
        let parts: Vec<&str> = base.split(',').map(str::trim).collect();
        return parts.get(version_index).copied();
    }
    Some(base.as_str())
}

fn num(value: Option<&str>) -> i32 {
    let Some(value) = value else { return 0 };
    value.trim().trim_start_matches('+').parse::<i32>().unwrap_or(0)
}

/// `mdmg` is written on the wiki as a percentage (e.g. `"3%"`, occasionally `"2.5%"`) - stored
/// as tenths of a percent (30 = 3.0%) so the value round-trips exactly without floats.
fn magic_damage_tenths(value: Option<&str>) -> i32 {
    let Some(value) = value else { return 0 };
    let trimmed = value.trim().trim_end_matches('%').trim();
    let Ok(percent) = trimmed.parse::<f64>() else {
        return 0;
    };
    (percent * 10.0).round() as i32
}

fn bonuses_from_params(params: &HashMap<String, String>, version_index: Option<usize>) -> RawBonuses {
    let get = |key: &str| get_param(params, key, version_index);
    let speed = get("speed");
    RawBonuses {
        attack_stab: num(get("astab")),
        attack_slash: num(get("aslash")),
        attack_crush: num(get("acrush")),
        attack_magic: num(get("amagic")),
        attack_ranged: num(get("arange")),
        defence_stab: num(get("dstab")),
        defence_slash: num(get("dslash")),
        defence_crush: num(get("dcrush")),
        defence_magic: num(get("dmagic")),
        defence_ranged: num(get("drange")),
        melee_strength: num(get("str")),
        ranged_strength: num(get("rstr")),
        magic_damage: magic_damage_tenths(get("mdmg")),
        prayer: num(get("prayer")),
        attack_speed: speed.map(|s| num(Some(s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_infobox_bonuses() {
        let block = "Infobox Bonuses\n|astab = 0 |aslash = 82 |acrush = 0 |amagic = 0 |arange = 0\n\
|dstab = 0 |dslash = 0 |dcrush = 0 |dmagic = 0 |drange = 0\n\
|str = 82 |rstr = 0 |mdmg = 0 |prayer = 0 |speed = 4\n\
|id = 4587";
        let params = parse_template_params(block);
        let version_index = find_version_index(&params, 4587);
        let bonuses = bonuses_from_params(&params, version_index);
        assert_eq!(bonuses.attack_slash, 82);
        assert_eq!(bonuses.melee_strength, 82);
        assert_eq!(bonuses.attack_speed, Some(4));
    }

    #[test]
    fn resolves_multi_version_item_by_numbered_id_suffix() {
        let block = "Infobox Bonuses\n\
|id1 = 100 |id2 = 200\n\
|astab1 = 5 |astab2 = 15\n\
|str = 10";
        let params = parse_template_params(block);
        let version_index = find_version_index(&params, 200);
        assert_eq!(version_index, Some(1));
        let bonuses = bonuses_from_params(&params, version_index);
        assert_eq!(bonuses.attack_stab, 15);
        // Non-suffixed params fall back to the shared value for every version.
        assert_eq!(bonuses.melee_strength, 10);
    }

    #[test]
    fn parses_magic_damage_percent() {
        assert_eq!(magic_damage_tenths(Some("3%")), 30);
        assert_eq!(magic_damage_tenths(Some("2.5%")), 25);
        assert_eq!(magic_damage_tenths(None), 0);
    }

    #[test]
    fn extracts_nested_template_blocks_without_truncating() {
        let wikitext = "{{Infobox Bonuses|examine=[[File:x.png|100px]] some {{plink|Whip}} text|astab=5}}";
        let blocks = extract_template_blocks(wikitext, "infobox bonuses");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("astab=5"));
    }
}
