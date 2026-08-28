# 19 [R]: REST request broker with declared schemas and streaming

**What to build:** Applications perform HTTP only through a host-owned broker governed by signed route-group declarations constraining origins, methods, paths, headers, response schemas, and generous explicit size/rate/timeout limits. Route groups declare a credential source: public, an OAuth provider-plugin session, or a named secret reference injected host-side. Declared routes support server-sent-event streaming responses delivering validated typed chunk events with cancellation and host-owned reconnect policy.

**Blocked by:** 18

**Status:** implemented-pending-verification (UNVERIFIED: written code-only; the serialized runner must confirm `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`)

- [ ] Requests to undeclared origins or paths are denied with stable codes
- [ ] Responses failing declared schema validation never reach guest memory
- [ ] Credential values appear in neither guest memory nor any log/diagnostic surface
- [ ] Streaming route delivers incremental validated chunks, honors cancellation, and applies declared bounds
- [ ] Integration suite executes against an approved real endpoint, not a simulator

## Implementation notes (UNVERIFIED)

New crate `crates/studio-net` (workspace member; host-owned networking lives here rather than in
`crates/studio-host`, which ticket 20 owns). Branch also fast-forward merges
`tt/18-protected-secret-store` so the broker consumes the landed seam.

- `src/declaration.rs` — signed route-group declaration schema (`RouteGroupDeclaration`, camelCase,
  `deny_unknown_fields`, schemars derives matching protocol conventions): origins normalized to
  scheme+lowercase-host+port (`Origin::parse`), closed method catalog (`HttpMethod`), path patterns
  with literal / `{slot}` / trailing `**` segments (`PathPattern`), allowed request-header tokens,
  credential source enum (`public | oauthProviderSession | namedSecret{name,header,prefix}`),
  streaming declaration with chunk schema + reconnect policy, per-group `DeclaredLimits`.
  `compile()` defensively re-validates everything against host ceilings and yields a parsed
  `CompiledRouteGroup` (schemas pre-built, headers lowercased, secret reference validated as a
  `ProtectedSecretKey`). Signature verification stays upstream in package signing; this module is
  fail-closed on any malformed input.
- `src/limits.rs` — generous explicit host ceilings/defaults (1 MiB request, 8 MiB response, 30 s
  timeout, 120 req/min sliding window, 64 MiB stream bytes, 100k chunk events, 1 h stream lifetime)
  with declaration-side narrowing that may never exceed a ceiling.
- `src/schema.rs` — bounded closed JSON-Schema subset validator (type, properties, required,
  additionalProperties defaulting to closed, items, enum, length/count/numeric bounds). Unknown
  keywords are rejected at declaration admission, never ignored.
- `src/admission.rs` — ordered stable denials: `net.route.origin_not_declared`,
  `path_not_declared`, `method_not_allowed`, `header_not_allowed`; header values are not inspected
  at admission time.
- `src/credential.rs` — object-safe `NamedSecretInjector` adapter over the sealed
  `BrokerSecretInjectionHandle` (blanket impl; broker holds `Arc<dyn NamedSecretInjector + 'store>`
  behind interior mutability so store borrows stay explicit); typed `OAuthSessionResolver` hook —
  unwired provider-session groups answer `net.credential.oauth_session_unavailable` (ticket 21
  fills it). `HeaderInjectionSink` composes prefix+secret into one header inside the
  `inject_at_send_time` call and registers the value with `SensitiveValueFilter` so no later
  diagnostic can echo it; non-UTF-8 secrets fail closed.
- `src/broker.rs` — pipeline: admit → sliding-window rate accounting → body bound + declared
  schema validation → send-time credential resolution → transport under effective timeout/response
  bound → status-window check → JSON parse → declared response-schema validation → registered-
  credential echo rejection (`net.response.sensitive_rejected`) → typed `TypedResponse{status,
  body}`. Every error detail passes through the scrubber before construction.
- `src/streaming.rs` — pump thread per stream: byte-level SSE framing (`\n\n` / `\r\n\r\n`,
  1 MiB frame cap), `data:`/`id:` handling (server `retry:` hints never override host policy),
  chunk JSON parse + bounded-schema validation + credential-echo check BEFORE any queue push,
  total-bytes/event-count/lifetime bounds, cooperative cancellation between reads, host-owned
  reconnect with exponential backoff and Last-Event-ID replay header, terminal events
  `Completed | Failed(code) | Cancelled`.
- `src/guest.rs` — only surface guests receive: `GuestRestApi` (execute/open_stream),
  `BrokerRequest`, `TypedResponse`, `StreamHandle` (next_event/cancel), `StreamEvent`. No socket,
  transport, header-map, or raw HTTP type crosses this module.
- `src/transport.rs` — `HttpTransport` trait (sync execute/open_stream) plus bounded
  `OutgoingRequest`/`IncomingResponse`/`ByteStream`; production client wiring is intentionally
  left to the host integration milestone (TODO in `Cargo.toml` feature note).
- Tests (`crates/studio-net/tests/`, deterministic scripted transport + in-memory credential
  backend): `admission_matrix.rs`, `response_pipeline.rs`, `credential_injection.rs`
  (fake backend, prefix injection, missing/revoked/undeclared names, scrubbed diagnostics, echoed-
  credential rejection), `streaming.rs` (framing across reads, invalid chunk, schema-violating
  chunk, cancellation interleaving, event-count and byte-budget bounds, reconnect with
  last-event-id, exhausted policy, streaming/non-streaming mismatch), `limits.rs` (rate window,
  oversized body, schema-invalid bodies, timeout/failure mapping, OAuth seam fail-closed +
  wired stub), `declaration_validation.rs` (unknown wire fields, unknown schema keywords,
  ceiling violations, GET-only streaming, duplicates).
- Real-endpoint suite stubbed behind `--features integration-real` AND
  `STUDIO_NET_REAL_ENDPOINT_URL` (`tests/integration_real_endpoint.rs`) with clear TODO for the
  approved endpoint and production TLS story; deterministic runner runs without it.

Known follow-ups (not blockers): per-reconnect credential re-injection (currently injected once
per stream open and reused across host-owned reconnects within one stream lifetime); SSE `event:`-
typed delivery is declared host-side only.
