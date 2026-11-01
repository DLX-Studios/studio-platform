# 35 [R]: Plugin/extension registry foundation

**What to build:** Registry validating signed plugin descriptors — identity, publisher, version, compatibility, contributions, requested capabilities — with consent-based admission, bounded lifecycle hooks, and contribution loading for Reusable Compositions, typed settings schemas, and commands/actions. Third-party plugins can never add native renderer kinds or raw GPUI access.

*Note: refines open grilling issue 07; descriptor scope may adjust when it resolves.*

**Blocked by:** 18, 22

**Status:** registry foundation implemented/tested; expanded contributions and native install UX remain partial

- [ ] First-party vertical pack installs, providing compositions and settings groups to a project
- [ ] Tampered or incompatible descriptors are rejected before activation
- [ ] Capability requests require explicit consent, are recorded per project, and are revocable
- [ ] A failing lifecycle hook is contained with bounded time/memory
- [ ] Removal reports remaining artifacts owned by the extension before changing the project

## Completion audit — 2026-11-01

Current source/evidence assessment: F08/F12 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
