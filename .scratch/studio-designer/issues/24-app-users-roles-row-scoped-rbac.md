# 24 [R]: Application users, roles, and row-scoped authorization

**What to build:** Generated applications support their own users — employee PIN/badge entry, email/password — verified entirely host-side. Roles bind to routes, screens, actions, and individual data records; row-scoped access is enforced at the data layer so bypassing the UI gains nothing.

**Blocked by:** 15

**Status:** ready-for-agent

- [ ] A technician-scoped principal requesting another employee's tickets is denied host-side, including through direct collection-helper calls
- [ ] PIN verification works fully offline against the app's user store
- [ ] Role and membership changes are auditable operations
- [ ] Repeated failed verification applies the declared throttle/lockout policy
- [ ] Enforcement has no dependency on interface visibility
