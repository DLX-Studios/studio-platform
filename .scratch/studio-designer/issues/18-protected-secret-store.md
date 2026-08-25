# 18 [R]: Protected secret store and protected configuration

**What to build:** One host-owned credential store partitioned per verified application identity, backed by the operating-system credential facility. Packages declare secrets by name and purpose only; publisher-configured values arrive out-of-band, user-supplied values through a host-owned entry surface. Guests observe status metadata only; injection happens exclusively inside network brokers; rotation never requires rebuilding or re-signing packages.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Cross-partition reads between applications are impossible through any interface
- [ ] No guest-callable read path for secret values exists; only configured/missing/revoked status
- [ ] User-supplied key entry captures the value without ever displaying it again
- [ ] Rotation replaces values while package signatures stay valid
- [ ] Release gate proves shipped artifacts contain no default credentials and known defaults fail authentication
- [ ] Redaction scrubs key-shaped values from logs, diagnostics, and crash reports
