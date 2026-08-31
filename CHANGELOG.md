# Changelog

All notable changes to GroupScape web are logged here, newest first.

## [1.0.237] - 2026-08-31

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

## [1.0.65] - 2026-08-28

### Removed
- Removed farming patch and bird house timers tracking from the site and Timers page.

## [1.0.64] - 2026-08-28

### Added
- Deploys now automatically post a summary of what changed to Discord.
