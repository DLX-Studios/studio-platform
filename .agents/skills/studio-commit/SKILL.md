---
name: studio-commit
description: "Checkpoint coding-agent changes in project docs, including canonical app briefs and agent guidance; initialize a missing STUDIO.md with studio-init, then commit and push scoped work."
---

# Studio Commit

Turn the current coding-agent session into an accurate, useful project checkpoint. This skill works across Codex and other coding-agent chats. Focus on changes that actually happened and the work's current state; do not use this skill to design the application or create its initial project structure. Use `$studio-init` for project discovery and application structure. `/compact` preserves useful working state but does not update repository files.

## Capture the work

- Summarize the user's objective, decisions, completed changes, validation actually performed, and unresolved work.
- Inspect the repository and its applicable instructions, then compare the working tree and index with the current commit. Treat a compacted chat summary as potentially incomplete; verify details against files and diffs. Mark information as unknown when it cannot be recovered.
- Keep project documentation about the project. Do not copy a full chat transcript or add temporary session notes to durable instruction files.

## Update project documentation

- First locate and read `STUDIO.md` at the repository root or under `docs/`, and follow it as project-specific guidance. If neither location has one, invoke the available `studio-init` skill by name before continuing the checkpoint; if it is not loaded, locate it through the current skill catalog. Resolve any project-shaping questions it raises, let it create the guide, then resume this checkpoint. Do not skip initialization or create a generic `STUDIO.md` in this skill.
- Find the project's existing changelog, including case variants such as `CHANGELOG.md` or `changelog.md`, and follow its format. If none exists, create a root `CHANGELOG.md` with an `Unreleased` section. Record concise, factual bullets for meaningful changes from this session; do not claim unperformed validation.
- Update documentation that explains the changed behavior or current project state, such as the relevant README or focused files under `docs/`. Keep changes targeted and consistent with existing style.
- Once a guide exists, update only relevant project facts affected by this checkpoint. If both root and `docs/` guides exist, put information where it belongs rather than duplicating whole sections. Leave structural changes and broader guide redesign to `$studio-init`.
- For app changes, check the app's `APP.md` and `AGENTS.md`. If stack, framework, runtime, target platforms, purpose, boundaries, or linked specs changed, update the canonical `APP.md` when its metadata or references are affected; keep `AGENTS.md` and related docs aligned when app workflows or spec-driven constraints changed. Create either file when required by root `STUDIO.md` and missing for an affected app. Treat the Console catalog as an index of `APP.md`; do not leave a separate, conflicting metadata copy.
- Find other applicable `AGENTS.md`/`agents.md` files in the repository. Update them only when this work changes durable instructions for future agents. Do not use them as changelogs. Create app-level files when required by project guidance; do not add unrelated agent files.
- Put important remaining work or decisions in the relevant project-state documentation when useful. Otherwise include them in the final handoff summary; do not invent a new status document without a project convention for one.

## Stage, commit, and push

An explicit `$studio-commit` invocation or direct request to add, commit, and push authorizes those operations for this checkpoint only. Do not treat automatic skill selection or a generic request to summarize/compact a chat as Git authorization; in that case, prepare the documentation and show the exact files, commit message, and upstream, then ask before staging or publishing.

- Before staging, inspect the branch, upstream, staged diff, unstaged diff, and untracked files. Never use `git add -A` or stage unrelated work. Leave pre-existing unrelated staged, unstaged, and untracked changes untouched.
- Stage only files changed for this checkpoint. If a file mixes this work with unrelated edits and they cannot be separated confidently, do not commit that mixed change.
- Create a descriptive commit for the scoped changes. Do not amend commits, rewrite history, or create an empty commit.
- Push the new commit only to the current branch's configured upstream, and only when doing so will not publish earlier unrelated commits already ahead of that upstream. If there is no upstream or the push would include unrelated commits, stop before pushing and report what configuration or decision is needed. Never invent a remote, set an upstream, force-push, or rewrite history.
- If a push fails, keep the commit and report the failure; do not retry by rebasing, changing remotes, or force-pushing.

## Report the checkpoint

After the documentation and Git checkpoint are complete, use the host's native `/compact` command if it is available to the agent. Do not try to run it as a shell command. If the host does not expose it to the agent, tell the user to run `/compact` manually when available; do not claim compaction happened unless it did.

Summarize the actual changes and documentation updated, the commit ID and push result, validation performed, and remaining work. Clearly distinguish completed, pending, and unknown items.
