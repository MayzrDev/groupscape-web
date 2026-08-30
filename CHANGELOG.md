# Changelog

All notable changes to GroupScape web are logged here, newest first.

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
