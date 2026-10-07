# 60 [R]: Provider-generic OAuth host wiring

**What to build:** `studio-app` launches OAuth for exactly one provider today. `prepare_github_services` returns early unless the admitted plan contains `github`, resolves only the `github` integration config, builds a `github`-literal `ProviderPackage`, and the action router accepts only the `github.oauth` capability before calling `sign_in("github")`. Meanwhile `OAuthManager` already accepts any number of provider packages and `sign_in` is already per-provider, and `ProviderRegistry::maintained()` catalogues eight providers. Make the launch path provider-generic: derive the provider id and integration configuration from the admitted plan and manifest, enable every declared provider on the manager, route sign-in actions by a `{provider}.oauth` capability, and provision client secrets under the `{provider}.oauth.client_secret` protected-configuration convention. Provider literals to remove: the plan filter and integration lookup in `host.rs`, the `ProviderPackage::new("github", ...)` construction, the capability gate and `sign_in("github")` literal in `plugin_surface.rs`, and the hardcoded secret name in `main.rs`. Route-group installation is already plan-driven through `ProviderAdmissionPlan::install_into` and needs no change.

**Blocked by:** 21

**Status:** ready-for-agent

- [ ] A test manifest declaring a non-GitHub maintained provider admits, configures, and reaches sign-in through the action router without live credentials
- [ ] No provider literal remains in the launch path; provider identity flows from the admitted plan and manifest
- [ ] Client-secret provisioning follows `{provider}.oauth.client_secret` and rejects a manifest whose declared secret name does not match the convention
- [ ] A provider absent from the maintained catalog fails closed at launch with a safe code, never a partial launch
- [ ] An application declaring multiple providers launches each independently; one provider failing to configure does not block the others
- [ ] `cargo clippy --locked -p studio-app --lib -- -D warnings` and focused `studio-app` tests pass