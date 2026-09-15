# Production HTTP broker boundary

All package REST traffic is host-owned. An admitted package supplies route declarations to
`StudioHost::prepare_broker`, which atomically compiles the origin, method, path, header, schema,
credential, and limit policy into one `RestBroker`.

The broker accepts an `Arc<dyn HttpTransport>`. Production hosts wrap their certificate-validating
TLS client in `ProductionHttpTransport`; deterministic tests inject an `HttpsClient` fake. The
production wrapper rejects plaintext URLs before the client is called and enforces response and
stream byte ceilings. It passes bounded connect, write, and read deadlines to the TLS client.

`HttpsClient` implementations must validate the server certificate and hostname, refuse redirects
outside the admitted origin, and honor every deadline. Request headers and bodies are borrowed by
the call and must not be retained or logged. Credential values are injected only immediately
before transport invocation by the protected-secret/OAuth resolver seams.

Transport failures are reduced to the closed `TransportError` family. The broker maps those to
stable guest-safe diagnostics; endpoint details, request headers, and credential material never
appear in those diagnostics.

The deterministic `production_transport` test exercises the plaintext rejection and response/SSE
limit paths without opening a socket or embedding a credential.
