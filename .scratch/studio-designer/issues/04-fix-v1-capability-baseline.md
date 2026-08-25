# Fix the Studio Designer v1 capability baseline

Type: grilling
Status: resolved

## Question

What exact user-visible capabilities must Studio Designer v1 include for the full authoring-to-runtime journey, and what explicit acceptance boundary prevents later enhancements from silently expanding the first implementation effort?

## Answer

Studio Designer v1 must author, inspect, preview, validate, and compile every component kind declared by the approved Studio Runtime protocol catalog. Completion for a kind requires its full property schema, semantic native renderer, Designer inspector, input/event behavior, accessibility behavior, responsive behavior, preview/Runtime parity, and automated evidence. A generic `Box` is the supported div-equivalent primitive; unknown kinds never silently fall back to it.

Designer must author and validate the complete Canvas profile matrix: phone, foldable, tablet, laptop, desktop, ultrawide, television, and 4K, with breakpoints at base, 480, 640, 768, 1024, 1280, 1536, 1920, 2560, and 3840. Profiles retain orientation, pixel ratio, safe areas, touch, pointer, hover, reduced motion, keyboard, and remote-focus semantics. The desktop Designer previews every profile; generated variants remain in the package, while native execution certification follows the Runtime hosts that actually exist.

Studio Library accepts and preserves common originals with provenance and deterministic runtime variants: PNG, JPEG, WebP, GIF, AVIF, sanitized SVG; MP4, WebM, MOV; MP3, WAV, Ogg, FLAC, M4A/AAC; WOFF2, WOFF, TTF, OTF; PDF, plain text, and Markdown; and SVG/raster icons. Unsupported codecs produce diagnostics. Visible/playable runtime use requires an approved primitive and host decoder.

Declarative applications support navigation, local state, conditions, repeated content collections, forms, overlays, and approved host actions. V1 includes the complete sandboxed extension registry: descriptor validation, lifecycle, primitive composition, inspector declarations, commands, actions, content types, validators, migrations, and capability requests. Third-party extensions do not add native renderers.

V1 also expands Runtime with host-mediated Stripe, OAuth, application data, REST, and WebSocket capabilities. Guests receive no sockets, provider credentials, OAuth tokens, or database connections. One logical host-owned embedded SurrealDB Runtime Data Store contains Runtime-managed Application Data Namespaces derived from verified publisher/application identity. Runtime exposes typed `data.collection` helpers and an explicitly declared bounded `data.surreal.query` capability; raw queries cannot switch namespace/database, access Studio system data, or enable forbidden network/scripting behavior. Signed migrations use a separate lifecycle path.

The reference POS journey must cover multiple screens, reusable compositions, Library assets/content/bindings, responsive profiles, navigation/state/repeated content/forms/overlays, live agent edits and grouped undo, offline persistence, optional cloud sync, extensions, preview, compilation, signing, and launch. It extends the existing POS example with Stripe sandbox payments, real OAuth sandbox flow, scoped embedded application data, a schema-constrained REST call, and a host-owned WebSocket session.

External integration acceptance uses real official sandbox/test endpoints and provided credentials; release cannot pass without live suites. Deterministic tests remain mandatory for pure command, validation, admission, redaction, retry, serialization, and conflict logic. Real containerized SurrealDB is permitted; invented provider/database fakes do not satisfy integration acceptance. Production payment/provider certification remains a separate operational and legal gate.

Anything not required to complete this authoring-to-runtime journey is a later feature. This expanded baseline requires new Runtime capability specifications and threat-model updates because the current implementation admits only simulated payment/printing and explicitly excludes generic network and persistent guest storage.

## Revision — 2026-08-25

The host-mediated Stripe decision above was replaced. Applications call official Stripe test-mode APIs directly through the schema-constrained REST broker using restricted test keys held as protected configuration; there is no bespoke `stripe.*` capability, and `payment.simulate` remains only until the real journey retires it. OAuth stays host-owned but becomes a first-party provider-plugin model in the style of Better Auth: versioned declarative provider descriptors maintained by Studio and shipped with the Runtime SDK, enabled per package by declaration with client ID/secret in protected configuration, flows and token storage executed by the host, claims exposed to the application. REST and WebSocket brokers keep generous explicit limits. SurrealDB embedding is confirmed. Hard-coded POS UI leaves the generic render path and returns as vertical component packages (Reusable Compositions, extension contributions, or Studio Script SDK libraries). A GitHub-viewer OAuth proof application precedes the POS journey. The specification's Runtime prerequisites section is authoritative where wording differs.
