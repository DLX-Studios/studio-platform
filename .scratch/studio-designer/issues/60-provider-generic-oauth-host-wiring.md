# 60 [R]: Provider-generic OAuth host wiring

**What to build:** `studio-app` launches OAuth for exactly one provider today. `prepare_github_services` returns early unless the admitted plan contains `github`, resolves only the `github` integration config, builds a `github`-literal `ProviderPackage`, and the action router accepts only the `github.oauth` capability before calling `sign_in("github")`. Meanwhile `OAuthManager` already accepts any number of provider packages and `sign_in` is already per-provider, and `ProviderRegistry::maintained()` catalogues eight providers. Make the launch path provider-generic: derive the provider id and integration configuration from the admitted plan and manifest, enable every declared provider on the manager, route sign-in actions by a `{provider}.oauth` capability, a…

**Outcome:** the launch path names no provider. `studio-package` gains an `identity_only` admission descriptor per provider, so an identity provider enables sign-in without granting a REST route group, and `maintained_oauth_descriptors()` is the single list the host provisions secrets from. `studio-app` builds one `ProviderPackage` per admitted provider onto one `OAuthManager`, and the action router resolves a `{provider}.oauth` capability against the maintained provider ids, answering the generic approved claims for every provider. The GitHub payload keeps its repository list, because the first-party GitHub viewer reads it; that is the one provider literal left, and it is in the action router rather than the launch path. A Microsoft package must state the tenant issuer it expects, because the token names the tenant that issued it.

**Blocked by:** 21

**Status:** done

- [x] A test manifest declaring a non-GitHub maintained provider admits, configures, and reaches sign-in through the action router without live credentials (`plugin_launch.rs`, Discord)
- [x] No provider literal remains in the launch path; provider identity flows from the admitted plan and manifest
- [x] Client-secret provisioning follows `{provider}.oauth.client_secret` and rejects a manifest whose declared secret name does not match the convention
- [x] A provider absent from the maintained catalog fails closed at launch with a safe code, never a partial launch
- [x] An application declaring multiple providers launches each independently; one provider failing to configure does not block the others
- [x] `cargo clippy --locked -p studio-app --lib -- -D warnings` and focused `studio-app` tests pass
