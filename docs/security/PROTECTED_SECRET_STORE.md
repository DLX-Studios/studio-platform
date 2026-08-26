# Protected secret store

Studio keeps runtime application credentials out of projects and packages. A package declares only
the credential `name` and its host-visible `purpose`. Values arrive through a host-owned entry
surface or an out-of-band publisher/install channel and are stored by the operating system.

## Authority boundaries

`ProtectedSecretStore` owns one credential backend and derives opaque partitions from the verified
publisher identity, application ID, and deployment environment. The digest is domain-separated and
length-delimited. Signing key, bundle digest, and runtime instance are excluded, so a publisher can
rotate its signing key or update/restart an application without moving credentials. Development
principals cannot address staging or production partitions.

The store creates two non-convertible capabilities:

| Capability | Receiver | Surface |
| --- | --- | --- |
| `GuestSecretStatusHandle` | Wasm/SDK adapter | Declared name, purpose, `missing`/`configured`/`revoked`, rotation revision |
| `BrokerSecretInjectionHandle` | Host REST/WebSocket broker | Borrowed bytes inside `inject_at_send_time` only |

The sealed guest trait has no value type, backend accessor, callback, or conversion to the broker
handle. Undeclared names are rejected. A handle is bound to one partition when the verified host
principal is admitted, so no call accepts a guest-selected publisher, application, or environment.

Values move through `SecretInput` and `CredentialBytes`, neither of which implements `Clone`,
`Display`, serialization, or value-bearing `Debug`. Credential and broker errors contain stable
codes only. Configured records and revoked markers are encoded only inside the OS credential
facility; they are never written to Studio Design, package archives, project databases, sync data,
backups, diagnostics, or history.

## Shipped credential facilities

The production adapter uses `keyring` 4.1's platform implementation:

| Target | Shipped facility | Operational requirement |
| --- | --- | --- |
| macOS | Keychain Services | User keychain must be available/unlocked |
| Windows | Windows Credential Manager | User credential vault must be available |
| Linux and other supported desktop Unix | freedesktop Secret Service over D-Bus | A Secret Service provider and user session bus must be available/unlocked |
| iOS and Android | None in this release | Initialization fails closed |

`OsCredentialBackend::is_available` reports adapter initialization only; an individual operation can
still fail closed when a vault is locked or unavailable. Automated tests use deterministic
`CredentialBackend` implementations and never depend on a developer or CI machine's real vault.

## Rotation and revocation

Rotation overwrites the credential-facility record and increments safe partition-local metadata.
It does not modify the signed declaration, so package signatures remain valid. Revocation replaces
the value with a persistent value-free marker. Purge removes that marker and returns the status to
`missing`.

Known default credentials are denied by SHA-256 digest at capture and checked again immediately
before broker injection. Raw defaults are not compiled into production code. The release-gate tests
also scan the compiled production library and signed package fixture for default or configured
credential bytes.

## Redaction

`SensitiveValueFilter` still removes explicitly registered values and opaque handles. It also
scrubs common authorization/key labels, provider-shaped tokens, JWT-shaped tokens, and nested JSON
fields such as `apiKey`, `clientSecret`, `authorization`, and `password`. Persistence validation
rejects content whenever the scrubber would change it; it does not persist ambiguous redacted data.
