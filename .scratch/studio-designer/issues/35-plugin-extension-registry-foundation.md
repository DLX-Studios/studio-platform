# 35 [R]: Plugin/extension registry foundation

**What to build:** Registry validating signed plugin descriptors — identity, publisher, version, compatibility, contributions, requested capabilities — with consent-based admission, bounded lifecycle hooks, and contribution loading for Reusable Compositions, typed settings schemas, and commands/actions. Third-party plugins can never add native renderer kinds or raw GPUI access.

*Note: refines open grilling issue 07; descriptor scope may adjust when it resolves.*

**Blocked by:** 18, 22

**Status:** ready-for-agent

- [ ] First-party vertical pack installs, providing compositions and settings groups to a project
- [ ] Tampered or incompatible descriptors are rejected before activation
- [ ] Capability requests require explicit consent, are recorded per project, and are revocable
- [ ] A failing lifecycle hook is contained with bounded time/memory
- [ ] Removal reports remaining artifacts owned by the extension before changing the project
