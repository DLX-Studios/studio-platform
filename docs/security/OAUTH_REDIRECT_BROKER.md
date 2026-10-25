# OAuth redirect broker

A hosted redirect broker lets a desktop application sign in with a provider that insists on an
exact registered redirect URI, without the application owning a fixed port or a public address.

The application binds an ephemeral loopback listener as it always does, and encodes the port it
bound into the `state` value it hands the provider. The provider redirects the browser to the
broker, the broker recovers the port from the `state` and answers `302` to
`http://127.0.0.1:{port}/oauth/callback`, and the browser follows it to the listener. The code,
the PKCE verifier, the token exchange, and the protected token vault never leave the machine.

Providers that accept a loopback redirect on their own — GitHub and Google among them — do not
need the broker at all. The host runs in loopback-only mode unless a broker is configured.

## The service

The service is the `studio-oauth-broker` binary. It holds nothing: no database, no disk, no
per-request state, and no log of any code, state, or token. It answers one request from the
request itself plus the shared secret.

| Variable | Meaning |
| --- | --- |
| `STUDIO_OAUTH_BROKER_SECRET` | Required. The secret shared with the applications that sign in through this broker. At least 32 bytes; the service refuses to start with a shorter one. |
| `STUDIO_OAUTH_BROKER_LISTEN` | Optional. The address to listen on, default `127.0.0.1:8080`. |
| `STUDIO_OAUTH_BROKER_APPLICATIONS` | Optional. Comma-separated application identifiers to serve. When unset, every syntactically valid identifier is served. |

The service speaks plain HTTP and is deployed behind a TLS-terminating reverse proxy. The
redirect URI registered with a provider is always `https://`, and the client refuses to
configure a broker whose base URL is not one, so the proxy is the only place a certificate
lives.

Build and run it with:

```text
cargo build --locked --release -p studio-oauth-broker
STUDIO_OAUTH_BROKER_SECRET="$(openssl rand -base64 48)" ./target/release/studio-oauth-broker
```

## The application

The host reads the broker from two environment variables. Both are optional: with neither set,
the host signs in over a loopback redirect exactly as it did before.

| Variable | Meaning |
| --- | --- |
| `STUDIO_OAUTH_BROKER_URL` | The public origin of the broker, for example `https://auth.example.com`. HTTPS only. |
| `STUDIO_OAUTH_BROKER_SECRET` | The same secret the broker was deployed with. |

The application identifier is the manifest's own id, so two applications never share a redirect
URI. The redirect URI a developer registers with a provider is:

```text
https://{broker}/cb/{manifest-id}
```

## Why the state is authenticated

The broker recovers the loopback port from the `state` value. A broker that merely parsed a port
out of the `state` would let anyone aim its `302` at an arbitrary service on the machine that
runs it: an open redirector wearing a loopback address.

So the port is covered by an authentication tag. The `state` is `port || tag || entropy`, where
the tag is `HMAC-SHA256(secret, port || entropy)` and the entropy is random per authorization.
The broker recomputes the tag and refuses anything it did not mint, so it only ever forwards to a
port an application proved it chose. A forged, tampered, or foreign `state` is refused with `400`
and nothing is forwarded.

The secret is configuration, not state. It is shared between an application and the broker and
changes when either is rotated; the broker still stores nothing per request.

## What the broker does not keep

- The `code`, the `state`, and the forwarded `Location` are zeroed as soon as the response is
  written.
- The request log carries the method, the path, and the status, and nothing else: no query
  string, no header, no body, and no forwarded location.
- A refusal body is fixed text. It never repeats anything the client sent.
