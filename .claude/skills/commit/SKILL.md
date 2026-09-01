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
4. Update `CHANGELOG.md` at the repo root. Headers are `## [x.y.z] - YYYY-MM-DD`, keyed by *date*, not by a 1:1 version-per-commit:
   - If the top block's date is already today, append the new bullets to the matching `###` subsection of that same block (creating the subsection if it doesn't exist yet). **Do not create a new versioned header for a same-day commit** — the version number in an existing header is not expected to track every commit that lands under it.
   - Otherwise (first commit of a new day), insert a new `## [x.y.z] - YYYY-MM-DD` block right after the `# Changelog` title line. Read the *current* version from `site/package.json` and add 1 to the patch number for this header — do not write that number to `package.json` yourself (see step 5).
5. **Never hand-edit the version field in `site/package.json`.** This repo's husky pre-commit hook (`npm run precommit` → `version:bump`) bumps the patch version and stages it automatically on every commit, unconditionally. Editing it yourself before committing causes a double bump (e.g. 1.0.287 → your manual 1.0.288 → the hook's 1.0.289, silently skipping the number you put in the changelog). Just predict the post-hook version (current + 1 patch) for the changelog header/commit message and let the hook do the actual write.
6. Stage `CHANGELOG.md` along with the rest of the changed files.
7. Create the commit exactly like the default commit flow (see global git-commit instructions: heredoc for the message, no `--no-verify`, author is the user only — never add a co-author).
8. Confirm with `git status` and check `site/package.json`'s version matches what you predicted in step 4 — if it doesn't, something bumped it more than once; fix forward with a correcting commit rather than amending.

## Example CHANGELOG.md block

```markdown
## [1.0.257] - 2026-09-01

### Added
- Live world map now shows a trail of each group member's recent movement.

### Fixed
- Prayer bar no longer flickers when a group member logs out mid-tick.
```
