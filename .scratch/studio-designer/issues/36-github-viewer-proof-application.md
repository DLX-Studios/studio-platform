# 36 [R]: GitHub viewer proof application

**What to build:** Small OAuth proof application preceding the flagship journey: sign in through the GitHub provider plugin, list repositories via authenticated declared REST routes, view a repository detail screen. Packaged, signed, launched in real Runtime. Establishes the `@studio/github` SDK package shape and the `@studio/ai` OpenAI-compatible streaming client skeleton for later use.

**Blocked by:** 21, 23

**Status:** ready-for-agent

- [ ] End-to-end sign-in and repo listing succeed against live GitHub
- [ ] Requests outside declared routes are denied even with valid session
- [ ] Package builds deterministically and launches in Runtime
- [ ] Documentation shows adding a second provider requires no application changes
