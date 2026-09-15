# Staging transport gate

The `studio-net` real-endpoint suite is intentionally disabled by default. It is compiled only
with `--features integration-real` and requires the approved staging contract below. Missing
configuration is a test failure, not a skip, so a release job cannot silently pass without reaching
the endpoint.

The job environment must provide:

| Variable | Meaning |
| --- | --- |
| `STUDIO_NET_REAL_ENDPOINT_URL` | HTTPS origin only (`https://host[:port]`), with no path or query. |
| `STUDIO_NET_STAGING_CREDENTIAL` | Staging-only credential injected by the protected-secret seam. Never print it. |
| `STUDIO_NET_STAGING_GET_PATH` | Declared GET route returning a JSON object. |
| `STUDIO_NET_STAGING_POST_PATH` | Declared POST route accepting `{"message":"staging-transport-gate"}` and returning a JSON object. |
| `STUDIO_NET_STAGING_SSE_PATH` | Declared SSE route that emits typed `{"text":"..."}` events and closes cleanly. |
| `STUDIO_NET_STAGING_RECONNECT_SSE_PATH` | SSE route that drops once after an event with an `id`, then completes after reconnect. |
| `STUDIO_NET_STAGING_STALLED_SSE_PATH` | SSE route that remains idle long enough to exercise the read deadline. |
| `STUDIO_NET_STAGING_OVERSIZED_PATH` | GET route returning more than 1 KiB of valid JSON. |
| `STUDIO_NET_STAGING_REJECTED_PATH` | GET route returning a non-2xx status (503 is recommended). |

`STUDIO_NET_CURL_BIN` may select a pinned curl binary; otherwise `curl` is used. Curl performs
certificate-validated TLS and HTTP/1.1. The adapter writes response headers and body to separate
short-lived temporary files, bounds both reads, and maps curl timeout/connection failures into the
closed transport error family. It never logs curl stderr or request headers.

Run the gate only in a staging job with outbound access and the variables injected by the secret
manager:

```text
cargo test --locked -p studio-net --features integration-real --test integration_real_endpoint
```

Do not put endpoint values, credentials, or captured responses in source, CI logs, artifacts, or
snapshots.
