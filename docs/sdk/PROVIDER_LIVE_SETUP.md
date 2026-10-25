# Provider live setup

What a developer registers at each provider, and what the host needs from them, to run a live
sign-in against the maintained catalog. The deterministic fakes in `provider_flow.rs` cover the
protocol; this covers the part only a real provider can answer: the redirect URI each one
accepts, and the artifact each one hands back.

## The two redirect shapes

The host binds an ephemeral loopback port on every launch, so the redirect URI it sends is
`http://127.0.0.1:{port}/oauth/callback`, where the port changes each time.

| Shape | What the provider is given | Who accepts it |
| --- | --- | --- |
| Loopback only | `http://127.0.0.1/oauth/callback` — registered **without** a port | GitHub, Google, Discord, Spotify |
| Brokered | `https://{broker-host}/cb/{app-id}` — one fixed URL | Apple, Meta, Microsoft, GitLab, LinkedIn |

The port may vary; the path may not. Every provider below that accepts a loopback compares the
path and ignores the port.

The brokered shape needs the broker service deployed and `STUDIO_OAUTH_BROKER_URL` and
`STUDIO_OAUTH_BROKER_SECRET` set in the host's environment. See
[OAUTH_REDIRECT_BROKER.md](../security/OAUTH_REDIRECT_BROKER.md).

## What the host needs from you

Everything below is per provider. **A client secret is never a value that goes into the
repository, into a manifest, or into a chat message.** It goes into the environment of the run
that signs in, and the host moves it into the protected store at launch.

| Provider | Artifact to create | Send me | Set in the run environment |
| --- | --- | --- | --- |
| `github` | OAuth App | the client id | `STUDIO_GITHUB_CLIENT_SECRET` |
| `google` | OAuth client, type **Desktop app** | the client id | `STUDIO_GOOGLE_CLIENT_SECRET` |
| `apple` | Services ID + a Sign in with Apple key (`.p8`) | the Services ID, the team id, and the `.p8` file | `STUDIO_APPLE_CLIENT_SECRET` (the key) |
| `discord` | Application → OAuth2 | the client id | `STUDIO_DISCORD_CLIENT_SECRET` |
| `facebook` | App → Facebook Login for Business | the app id, and the business verification it needs | `STUDIO_FACEBOOK_CLIENT_SECRET` |
| `gitlab` | Application (group or personal) | the application id | `STUDIO_GITLAB_CLIENT_SECRET` |
| `linkedin` | App → Products → Sign In with LinkedIn | the client id | `STUDIO_LINKEDIN_CLIENT_SECRET` |
| `microsoft` | App registration | the application (client) id **and the tenant id** | `STUDIO_MICROSOFT_CLIENT_SECRET` |
| `spotify` | App | the client id | `STUDIO_SPOTIFY_CLIENT_SECRET` |

Microsoft is the one that needs two values: the token names the tenant that issued it, so the
application must state the tenant issuer it expects.

## Redirect URI to register, per provider

| Provider | Register this | Notes |
| --- | --- | --- |
| `github` | `http://127.0.0.1/oauth/callback` | GitHub compares the host and path and **ignores the port** for a loopback URL. |
| `google` | `http://127.0.0.1/oauth/callback` | A Desktop-app client accepts a loopback with any port. |
| `discord` | `http://127.0.0.1/oauth/callback` | Discord's own desktop guidance registers `http://127.0.0.1/callback`; the path has to match the host's. |
| `spotify` | `http://127.0.0.1/oauth/callback` | Spotify requires a loopback **IP literal** with no port, and refuses `localhost` outright. |
| `apple` | `https://{broker-host}/cb/{app-id}` | The return URL must be HTTPS on a named domain. Apple will not accept a loopback URI, so the broker is the only path. |
| `facebook` | `https://{broker-host}/cb/{app-id}` | Exact match required, and Meta does not accept a loopback URI for a web app. |
| `microsoft` | `https://{broker-host}/cb/{app-id}` | A confidential web client needs HTTPS. A **Mobile and desktop applications** registration would accept `http://localhost/oauth/callback` with any port, but the Azure portal refuses an `http` + `127.0.0.1` entry in the text box: it has to be added by editing `replyUrlsWithType` in the application manifest. |
| `gitlab` | `https://{broker-host}/cb/{app-id}` | Exact match, or a relative path GitLab resolves against the app's domain. |
| `linkedin` | `https://{broker-host}/cb/{app-id}` | Exact match, and HTTPS. |

`{app-id}` is the **manifest id** of the application that signs in. For the first-party example
that is `com.studio.github-viewer`, so its brokered redirect URI is
`https://{broker-host}/cb/com.studio.github-viewer`.

## The scopes the host asks for

The host requests exactly the scopes its descriptor declares, and the host's own ceiling refuses
anything wider. Ask for these and no more:

| Provider | Scopes |
| --- | --- |
| `github` | `read:user`, `user:email` |
| `google` | `openid`, `email`, `profile` |
| `apple` | `name`, `email` |
| `discord` | `email`, `identify` |
| `facebook` | `email`, `public_profile` |
| `gitlab` | `email`, `openid`, `profile` |
| `linkedin` | `email`, `openid`, `profile` |
| `microsoft` | `email`, `offline_access`, `openid`, `profile` |
| `spotify` | `email`, `openid`, `profile` |

## A test-only broker secret

For a local run, the broker service and the host must be given the same value. This one is
test-only and is in this document on purpose:

```text
STUDIO_OAUTH_BROKER_SECRET=test-only-0d9f4c1a7b6e8d2f5a3c9e7b1d4f6a8c2e5b7d9f1a3c5e7b9d1f3a5c7e9b1d3f5a
```

It is 64 bytes, above the 32-byte minimum, and it is not a secret to anyone. A real deployment
generates its own: `openssl rand -base64 48`.

```text
cargo build --locked --release -p studio-oauth-broker
STUDIO_OAUTH_BROKER_SECRET="…" STUDIO_OAUTH_BROKER_LISTEN=127.0.0.1:8080 ./target/release/studio-oauth-broker
```

## What the evidence looks like

A live sign-in is recorded as the status code the provider answered with and the host's own
`oauth.*` result code. No authorization URL, no callback value, and no token is ever written
down: the host's transport already refuses to render a request that carries a credential, and
the broker's log carries only the method, the path, and the status.
