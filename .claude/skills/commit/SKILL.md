---
name: commit
description: Commit staged/unstaged changes in this repo, always updating CHANGELOG.md with a real entry first — used instead of a bare `git commit` so the changelog stays in sync with history and Discord patch notes have something worth reading.
---

# Commit (groupscape-web)

This repo's deploy pipeline posts the newest CHANGELOG.md entry straight to Discord's #patch-notes when the site updates. That embed is only as good as this entry, so write it for players/users, not for other developers — a commit message can say "refactor session middleware," a changelog entry has to say what changed for them, or (for a pure internal change) be honest that there's nothing user-facing.

## Steps

1. Run `git status`, `git diff` (staged + unstaged), and `git log -5 --oneline` in parallel to see what's changing and match this repo's commit message style.
2. Draft the commit message per the usual rules (see the global git-commit instructions) — why, not what, 1-2 sentences.
3. Draft the changelog entry:
   - 1-3 short bullets, written for someone using the site — "Group map now shows other members' world" not "fixed CanvasMap.js world sync bug".
   - Categorize each bullet under `### Added`, `### Changed`, `### Fixed`, or `### Removed`.
   - If the change is genuinely internal-only (refactor, test, CI, deploy tooling) with zero user-facing effect, still add one bullet under `### Changed` — keep it honest and brief (e.g. "Internal: tidied up deploy scripts") rather than skipping the entry. Every commit gets one.
4. Update `CHANGELOG.md` at the repo root:
   - If the top block's header is already today's date (`## YYYY-MM-DD`), append the new bullets to the matching `###` subsection (creating the subsection if it doesn't exist yet), instead of adding a second block for the same day.
   - Otherwise, insert a new `## YYYY-MM-DD` block right after the `# Changelog` title line (before any existing dated blocks) — newest always on top.
5. Stage `CHANGELOG.md` along with the rest of the changed files.
6. Create the commit exactly like the default commit flow (see global git-commit instructions: heredoc for the message, no `--no-verify`, author is the user only — never add a co-author).
7. Confirm with `git status`.

## Example CHANGELOG.md block

```markdown
## 2026-08-28
### Added
- Live world map now shows a trail of each group member's recent movement.
### Fixed
- Prayer bar no longer flickers when a group member logs out mid-tick.
```
