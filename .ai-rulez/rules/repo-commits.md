---
type: Decision
title: Repo Commits
x-ai-rulez:
  kind: rule
  id: repo-commits
  metadata:
    priority: high
---

- Update `CHANGELOG.md` `[Unreleased]` on every upstream commit; never merge a fix without an entry.
- Releases: run the `release-procedure` skill (`/release-procedure`).
- `--no-verify` only when a hook is broken; re-run the hooks afterwards.
- Never amend a commit that may be pushed or in another agent's HEAD; add a fix commit.
- Investigate unexpected files or branches before deleting them; they may be another agent's work.
