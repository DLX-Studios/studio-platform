# GitHub viewer proof application

The checked-in `examples/github-viewer` package is the smallest signed Runtime launch target for
the provider-plugin path. Its manifest pins the `github` integration descriptor, names the OAuth
client secret without containing its value, and signs three REST route groups:

| route group | method | path | credential |
| --- | --- | --- | --- |
| `github.user` | `GET` | `/user` | host-resolved GitHub session |
| `github.repositories` | `GET` | `/user/repos` | host-resolved GitHub session |
| `github.repository` | `GET` | `/repos/{owner}/{repo}` | host-resolved GitHub session |

`crates/studio-github` is the host-neutral typed SDK. `GithubClient` accepts only the restricted
`GuestRestApi`; it cannot receive an OAuth token. `GithubViewer` models the deterministic journey:
sign-in request, authenticated repository list, and repository detail. Browser handoff, callback
capture, token storage, and send-time credential injection remain host responsibilities.

`crates/studio-ai` and `sdk/ai` establish the provider-neutral OpenAI-compatible request and
validated SSE chunk shape. Streaming uses a declared, bounded POST adapter route carrying the
same ordered messages, temperature, and `stream_options` body as a non-streaming request;
applications do not gain a raw socket or an API-key path.

## Build and launch

With Bun and the pinned AssemblyScript toolchain installed:

```bash
bun run ./scripts/build-example.ts github-viewer
cargo run -p studio-app -- --dev examples/github-viewer/build/github-viewer.studio
```

Before a production launch, replace the manifest's example client id through the release
configuration workflow and provision `github.oauth.client_secret` through protected host storage.
The manifest never carries that secret.

## Adding a second provider

Applications depend on the generic `GithubRestClient`/`RestRequest` and `AiTransport` boundaries,
not provider-specific sockets or credential code. A second integration contributes another
versioned descriptor, package integration reference, and route groups. The viewer's application
state and typed screen journey remain unchanged; only the selected integration and its host
configuration change.

## Verification boundary

The deterministic SDK and route contracts are checked in, but live OAuth, GitHub API, signing, and
Runtime launch acceptance require provider credentials, a configured callback environment, and a
native Wayland session. Those external gates remain intentionally visible until exercised.
