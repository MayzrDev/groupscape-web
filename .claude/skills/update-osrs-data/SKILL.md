---
name: update-osrs-data
description: Refresh GroupScape's static OSRS item/boss data (item_data.json, item icons, boss levels, notable NPCs) from the OSRS Wiki after a game update. Use when the user says the item/boss data is stale, asks to check for new OSRS content, or wants this run after a known game update.
---

# Update OSRS data

GroupScape's item and boss data is a static, hand-maintained snapshot — there's no live fetch at runtime. This skill refreshes it from the OSRS Wiki. Item updates are mechanical; boss updates need judgment (is this new NPC "notable" enough for the activity feed? does it need a curated drop table?), so this skill does the mechanical part with scripts and the judgment part itself.

## Steps

1. **Fetch item data.** Run `npm run update-items` inside `site/` (`site/scripts/osrs-data/fetch-items.mjs`). This pulls the OSRS Wiki pricing API's item mapping, diffs it against `site/public/data/item_data.json`, updates changed/new entries, downloads+converts new icons to `site/public/icons/items/<id>.webp`, and mirrors the result to `server/src/content/item_names.json` (must stay byte-identical — the Rust server embeds it via `include_str!`). Report what changed: items added/updated, icons fetched, any download failures.

2. **Detect boss/NPC candidates.** Run `npm run detect-bosses` inside `site/` (`site/scripts/osrs-data/detect-new-bosses.mjs`). This is read-only — it scans the wiki's boss category against `site/src/data/boss-levels.js`'s `BOSS_COMBAT_LEVELS` and reports candidates not yet tracked, with combat level and whether wise-old-man already has a hiscore-style icon for it.

3. **Classify each candidate boss.** For every candidate in the report:
   - Decide if it belongs in `NOTABLE_NPCS` (`server/src/notable_npcs.rs`) — i.e. is it a real boss/quest-boss worth surfacing in the group activity feed, not feed noise (regular monsters, reused generic NPC names). Ask the user via `AskUserQuestion` when it's ambiguous (e.g. a minigame boss, a reused generic-sounding name).
   - If notable, add its slug to `NOTABLE_NPCS` and add a `slug: combatLevel` entry to `BOSS_COMBAT_LEVELS` in `site/src/data/boss-levels.js` — keep these two lists in sync, matching the existing hand-curated style and ordering in both files.
   - Only add a `drop_rates.json` entry (`server/src/content/drop_rates.json`) if the boss should get curated loot-rarity display in the loot log — this is a bigger, separate curation effort (item id → name/rarity/isUnique per drop), not something to rush through automatically. Ask the user whether they want to do this now for a given boss, or leave it for later.
   - Add the icon: if wise-old-man already has it (per the report), download `https://raw.githubusercontent.com/wise-old-man/wise-old-man/master/app/public/img/metrics/<slug>.png` to `site/public/icons/hiscore/bosses/<slug>.png`. Otherwise, fetch the boss's wiki infobox image and resize/convert to a 25×25 PNG with `sharp` to match the existing icon set's dimensions. Add the slug to `BOSS_ICON_SLUGS` in `site/src/data/boss-icons.js`.

4. **Validate.** Run `npm test -- cache-data.test.js` in `site/` and `cargo test notable_npcs` in `server/`. Fix anything that fails before continuing.

5. **Commit.** Use the `commit` skill (this repo's scoped one, not a bare `git commit`) to commit the changes with a real CHANGELOG.md entry — describe it in player-facing terms (e.g. "Added support for tracking <Boss Name> kills and drops") or as an internal data-refresh entry if nothing changed user-facing this run.

## Notes

- The item mapping endpoint only covers GE-tradeable items (~4,000 of ~19,500). Non-tradeable additions (most quest items, some cosmetics) won't be caught by `update-items` — if the user mentions a specific non-tradeable item is missing, add it to `item_data.json` (and mirror to `item_names.json`) by hand.
- There is no CI gate on this data anymore (`.github/workflows/cache-validation.yml` was removed) — the `npm test`/`cargo test` step in this skill is the only validation, so don't skip it.
