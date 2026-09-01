# Changelog

All notable changes to GroupScape web are logged here, newest first.

## [1.0.288] - 2026-09-01

### Fixed
- Activity Feed no longer gets stuck endlessly spinning for a group with a long same-boss farming streak - when merging repeat kills keeps pulling in more history without adding anything new to see, it now pauses and shows a "Load more" button instead of spinning forever.

## [1.0.257] - 2026-09-01

### Added
- Activity Feed now posts a standalone announcement when a member finishes every combat achievement task for a specific boss, the same way finishing a collection log page already does.
- Group Settings now has a Discord Integration section — paste in a channel webhook URL and GroupScape can post your group's kills, deaths, loot, notable drops, and raid completions straight to Discord, with a toggle for each one. Saving sends a live test message first, so a bad or deleted webhook is caught immediately instead of silently failing later.
- Discord boss kill notifications now show the total GE value of what the boss dropped, e.g. "killed Zulrah (KC: 45) — loot worth 1,204,300 gp".
- The Activity Feed now shows raid completions for Chambers of Xeric, Theatre of Blood, and Tombs of Amascut, including who finished it, the difficulty (invocation level for Tombs of Amascut, mode for the other two), and how much the reward chest was worth. If several of you finish the same raid together, it shows up as one entry crediting everyone instead of one per person. Raids also get their own filter chip and toast notification, and Tombs of Amascut shows a different icon depending on the invocation level reached.
- Graphs now has a "Raid Completions" option alongside XP and Boss KC, showing your group's Chambers of Xeric, Theatre of Blood, and Tombs of Amascut completions over time. Filter by raid and (for CoX/ToA) by difficulty, and switch between one combined group line or a line per member - the leaderboard panel ranks members by how many of those completions they were part of.
- Loot Log tiles now show a small helmet badge (tinted to each member's color) for who got the drop when "All members" is selected. An item dropped by more than one member is combined into a single tile with a stacked badge and a per-member breakdown in the tooltip, instead of one duplicate tile per member. Boss kill sources also get their chathead icon next to the name.
- Loot Log summary now shows a session card - the time span from your first kill to your last within the selected window, plus the duration.
- Each Loot Log source now shows its own first-kill-to-last-kill time range next to the kill count, so a boss farmed across several days shows that span instead of just a total.
- Clicking a Loot Log item now opens its OSRS Wiki page in a new tab.
- Loot Log boss names now show their combat level (e.g. "Vorkath (level-732)"), matching the in-game overhead format, for bosses with one fixed level.
- Group Settings' Discord Integration section now has toggles for Combat achievements, Collection log, Quest completions, and Diary completions, so those milestones can post to Discord too.

### Changed
- Activity Feed kill entries now show the boss's icon next to its name, and repeated kills of the same boss by the same member within an hour merge into one entry with a running count (e.g. "killed Vorkath ×3") instead of listing each kill separately - the same merging applies to a kill toast that's still on screen.
- Loot Log now groups drops by boss, chest, or clue tier under a square icon grid instead of a flat list, with a rarity-colored border on each item and a hover tooltip showing its name and value. Each source shows its own total value alongside a kill/open/casket count. Loot splitting and the per-person breakdown have been removed - Loot Log is just a log now.
- Loot Log item art now has a lot more breathing room inside its rarity border instead of crowding the edge.
- The Loot Log time window's "Last day" option is now labeled "Last 24h".
- Activity Feed entries for new collection log items now show the item's own icon next to the collection log icon, and link out to that item's OSRS Wiki page.
- Activity Feed's left-hand rail now filters by activity type (kills, raids, collection log, clues, etc.) and the pills at the top now filter by member - swapped from before, where it was the other way around.
- Discord's "Loot" and "Notable drops" toggles are now a single "Drops" toggle with an adjustable minimum value (250k gp by default) - only drops worth at least that amount get posted.
- Discord's "Kills" notification is now "Boss kills" and only fires for actual bosses, not regular NPCs, and now includes the member's kill count for that boss.
- Discord kill and drop messages now show the item's real name, linked to its OSRS Wiki page, instead of "item #12345".
- Loot Log now defaults to the last 24h instead of the last hour when you open it.
- Discord "Drops" messages now show each item's value (or "untradeable" if it has none) and its drop rate when known, and value now reflects the whole stack - 33x a 100gp item shows as 3,300 gp, not 100 gp.
- Loot Log's Source filter now only lists sources you've actually looted in the selected date range, instead of every boss and chest in the game - picking a different date range updates the list, and it's empty if nothing was looted in that range.

### Fixed
- Loot Log items dropped by a boss with no curated rarity data no longer show up as "Item #12345" - the real item name is shown.
- Loot Log items GroupScape doesn't recognize (no name, image, or value on file) no longer render as broken empty tiles - they're hidden instead.
- Loot Log items from the same kill no longer reshuffle their order every time the page refreshes.
- Coin drops in the Loot Log no longer value at 0gp.
- Loot Log boss, chest, and clue-tier icons now use the same small square icon as RuneLite's Hiscore panel (and the official OSRS hiscores page), self-hosted instead of hotlinked so they always load.
- Test/diagnostic kill data no longer shows up in the Loot Log.
- Untradeable items in the Loot Log now show an "Untradeable" tag in their tooltip instead of "0 gp x quantity".
- The "Exit view" button on the admin read-only viewing banner no longer fails to click while looking at the Map tab.
- Activity Feed's type and member filter counts no longer zero out every other option when you pick a filter - they now show real counts for each choice regardless of what's currently selected.
- The Discord notify toggles in Group Settings were invisible and couldn't be clicked at all - fixed.
- Kills, loot, and deaths could occasionally show up twice in the Activity Feed and get posted twice to Discord, most often right around a server restart - fixed.

## [1.0.254] - 2026-08-31

### Fixed
- Loot Log was always empty - every kill report from the plugin was silently rejected by the server, so no kill or loot ever got recorded. Kills (and their loot) now save properly.

## [1.0.253] - 2026-08-31

### Added
- Graphs now has 1H, 6H, and 12H time period options, alongside the existing 24H/7D/30D/1Y.

### Changed
- Simplified the top nav's account menu into a plain divider next to the tab row, instead of a floating panel — fixes it occasionally overlapping page content on Activity Feed, Combat Achievements, and Loot Log.

## [1.0.248] - 2026-08-31

### Changed
- GP Earned now tracks your total wealth - bank, inventory, and worn gear combined - instead of just your bank, so it shows a true day-to-day gp gain or loss.

### Fixed
- Boss KC now actually records kills. Previously most boss kills, especially ones with longer death animations, were never counted.

### Removed
- The Loot Value graph and leaderboard option, which never had any data, have been removed.

## [1.0.246] - 2026-08-31

### Added
- Raid markers on the map now include four numbered (1-4) and four lettered (A-D) generic callouts, alongside Danger/Defend/Loot/Focus.

### Changed
- Renamed the "Safe Spot" raid marker to "Defend".

[1.0.245] — Fix Menu Overlap On Activity Feed, Loot Log, Graphs Leaderboard

## [1.0.243] - 2026-08-31

### Fixed
- The target HP bar in the roster no longer looks short of full when you're talking to or banking with an NPC.

## [1.0.241] - 2026-08-31

### Changed
- The Activity Feed and Loot Log pages now use the full width of the screen instead of staying narrow and centered, matching the Items page.

## [1.0.238] - 2026-08-31

### Changed
- The loot log page is simpler now - the clue tier, sort, and accounting-mode filters are gone, and drops are always listed most-recent-first.

## [1.0.235] - 2026-08-31

### Added
- Clue casket completions now show up in the activity feed, with the clue tier, total casket value in gp, and a clue scroll icon.

## [1.0.233] - 2026-08-31

### Fixed
- The "Combat Achievements" tab in the top nav no longer gets crowded out by the account menu on narrower screens.

## [1.0.231] - 2026-08-31

### Fixed
- On mobile, the player stats panel could no longer be closed by tapping outside it without also blocking taps on the buttons and tabs inside the panel itself.

## [1.0.229] - 2026-08-31

### Changed
- The Combat Achievements member list and the group settings colour picker now show each member's actual helmet icon, tinted to their chosen colour, instead of a plain initial or a hand-drawn helmet shape.

## [1.0.227] - 2026-08-31

### Added
- The group map now shows raid markers (Danger, Safe Spot, Loot, Focus/Kill Target) dropped by teammates in-game, with the same icon they see in RuneLite.

## [1.0.226] - 2026-08-31

### Fixed
- Fixed clearing a ping (and pings ending automatically) silently failing every time.

## [1.0.225] - 2026-08-31

### Changed
- The ping toast now stays on screen for 20 seconds instead of 6, so there's actually time to click it before it disappears.

## [1.0.222] - 2026-08-31

### Added
- Dropping a ping now pops a quick toast for everyone in the group - click it to jump straight to that spot on the map.

## [1.0.221] - 2026-08-31

### Added
- Group members can now drop pings on the map that everyone in the group sees live, both in-game and on the website map, laying the groundwork for the upcoming right-click/hotkey ping feature in the RuneLite plugin.

## [1.0.71] - 2026-08-31

### Changed
- Internal: fixed the deploy pipeline so the Discord patch-notes bot can find the server's deploy folder, so it stops reporting "no changes" on every real release.

## [1.0.70] - 2026-08-31

### Fixed
- Fixed the RuneLite plugin's live party overlay and side panel not showing other group members, even though they showed up fine as online on the website. A routing mix-up on the server meant the plugin's real-time connection request and character-linking request were quietly being rejected.

## [1.0.69] - 2026-08-31

### Changed
- Internal: the live group vitals feed now also carries each member's map position, laying the groundwork for the upcoming RuneLite plugin feature that shows group members on the in-game world map and minimap.

## [1.0.68] - 2026-08-31

### Fixed
- Fixed a group member's active prayer icons sometimes staying visible after they logged out.

## [1.0.67] - 2026-08-30

### Fixed
- Fixed the account page's online badge showing a group member as offline after a minute of standing idle even though their client was still checking in fine.

## [1.0.66] - 2026-08-29

### Added
- Activity feed now shows icons for quest completions, combat achievement tiers, diary entries, and collection log additions.

### Changed
- Combat Achievements tab now fills the full page width like Graphs does, with bigger progress bars, per-tier percentages, and a group-average summary row.
- Character avatars and text on the Account > Characters page are bigger and easier to read.

### Fixed
- Fixed group members incorrectly showing as offline while idle (standing still, AFK skilling, etc.) even though RuneLite was still connected and sending updates fine.
- Added logging around the live party-overlay connection so future connection drops are easier to diagnose (no user-facing change).
- Group Settings no longer shows your personal account token in place of your group's real invite token when you open it via Account > Characters — the Copy button is disabled with an explanation until you reroll for a real, shareable one.
- Discord boss kill notifications now show your account's real in-game kill count instead of how many times GroupScape happened to see that boss's kill logged on its own server.

## [1.0.65] - 2026-08-28

### Removed
- Removed farming patch and bird house timers tracking from the site and Timers page.

## [1.0.64] - 2026-08-28

### Added
- Deploys now automatically post a summary of what changed to Discord.
