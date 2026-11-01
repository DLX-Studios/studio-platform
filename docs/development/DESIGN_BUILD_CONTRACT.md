# Immutable Designer build contract, version 1

Status: implementation contract for completion tasks C004–C006. This document does not
claim that Designer can already build or launch an application. The host owns admission,
compiler invocation, asset bytes, signing keys, publication, and Runtime launch. The
Designer domain supplies immutable source and receives safe diagnostics and receipts.

## Request

The closed request has these fields; unknown fields and unsupported versions fail admission.

| Field | Contract |
| --- | --- |
| `schema_version` | Exactly `1`. |
| `request_id` | Bounded operation identity for correlation and cancellation; excluded from artifact bytes. |
| `project_id` | Exact project identity in the authenticated ownership grant and Design snapshot. |
| `revision_id` | Exact immutable revision in `design`; it may not resolve to a later live session revision. |
| `design` | Complete `StudioDesignSnapshot`, including ordered screens, interactions, tokens, responsive variants, content definitions and source identities. |
| `library` | Immutable `LibrarySnapshot` mapping source asset IDs to normalized package paths. |
| `asset_digests` | Sorted map from each referenced package path to the SHA-256 digest of admitted immutable bytes. |
| `manifest` | Closed `studio-package` manifest, validated by the existing manifest parser. Asset declarations must exactly match packaged paths. |
| `target` | `native-v1`; platform-neutral protocol-v1 guest running in the native Runtime. Unsupported targets fail before compilation. |
| `signing_key_id` | Opaque reference to a host-configured publisher key. No seed, credential, file path or environment-variable expression is accepted from Design source. |

The host supplies the active authenticated session, cancellation token, bounded compiler
service, immutable asset reader, signing-key resolver, output store and Runtime launcher
as injected services. These are not serialized request fields. Project ownership is checked
before fetching source/assets and again before publishing or launching a result. Revoked
sessions may not publish a completed background build into another identity's shell.

## Validation and compilation

1. Verify ownership, schema/revision consistency, source structure, asset identity/digests,
   manifest permissions and supported target. Reject unresolved assets and path aliases.
2. Derive protocol trees through the same `studio-design` projection used by native preview.
   Validate every declared screen/profile, not just the selected canvas. Preserve source IDs.
3. Lower the complete supported Design behavior to the existing `studio-script` compiler and
   AssemblyScript backend. Share compiler functions with CLI builds instead of invoking shell
   commands assembled from source values. Invoke a pinned compiler executable with explicit
   argument arrays and bounded temporary paths when a subprocess is necessary.
4. Admit emitted module and protocol contracts before packaging. Reject an unsupported behavior
   with a node/property-linked diagnostic. A static screenshot guest, manually written WAT,
   first-screen-only export, or silent omission of interactions/content is not a successful build.
5. Call `studio_package::pack_bundle` using the validated manifest, exact compiled module,
   sorted admitted asset bytes, and the resolved host signing key. Designer v1 Build/Run has no
   implicit unsigned mode and never uses the repository's fixed example development seed.

The Script editor's `ScriptDocumentAdapter` is not presumed to be a complete build lowering:
its representation must be checked for screens, navigation, responsive overrides, interactions
and assets. Missing lowering produces an explicit unsupported diagnostic until implemented.
The same rule applies to projection warnings: unsupported source cannot become an apparently
successful artifact because the native canvas omitted it.

## Reproducibility and output

The build input digest covers the canonical Design snapshot, immutable Library mapping and
asset digests, canonical manifest, target, compiler/backend/SDK version identities, and publisher
public-key identity. Toolchain identity is host selected and recorded in the receipt. Request
IDs, temporary paths, wall-clock time, current canvas selection and mutable UI state are excluded.

Identical inputs and publisher key must produce byte-identical `.studio` archives on repeated
builds with the same toolchain. Output is staged in a host-owned temporary location. Only a
successful signed artifact replaces the last-good reference; a failed or cancelled build leaves
that reference intact. Receipts include project/revision IDs, input digest, artifact digest,
public-key ID, toolchain identity and normalized asset digest list. They contain no signing
material, raw credentials, unrestricted source dumps or native filesystem paths.

The shell may display a successful result from an older source revision as an explicitly older
build; it may not silently label that artifact current. Run uses the selected receipt's exact
bytes and digest. It does not rebuild from live source behind the user's back.

## Diagnostics, lifecycle and launch

Phases are `admit`, `lower`, `compile`, `package`, `publish`, and `launch`. Each diagnostic has a
stable code, severity, safe explanation, and optional source node/screen/property or Script span.
Keep backend details in sanitized host diagnostics. Admission and compile errors remain visible
in Designer and do not close the source session or discard its undo history.

Cancellation stops scheduling the next phase and terminates a running compiler child with a
bounded shutdown/reap path. Timeouts, maximum source/module/asset sizes and bounded subprocess
output are host policy. Completion carries the originating session/request/revision identity;
the shell discards stale callbacks after project changes or sign-out.

Run crosses the existing production Runtime package admission path, verifying signature,
publisher trust, manifest permissions and asset integrity before installing the guest. The
launched application has its own guest authority; the authoring session and signing key are
never passed to it. A launch failure preserves the build and reports a separate launch diagnostic.

## Required evidence

- Build the same two-screen Design twice and compare exact signed archive bytes.
- Launch that artifact with production admission and exercise real button/input/navigation events.
- Match node IDs, text, layout, token/profile values and asset paths to native Designer preview.
- Change source during a build and verify snapshot isolation and older-build labeling.
- Reject missing assets, digest mismatches, unknown schema/target, foreign project and expired session.
- Tamper with module, manifest, signature and asset bytes independently; Runtime admission must fail.
- Fail/cancel each phase, including a compiler subprocess; preserve last-good artifact and reap children.
- Cold reopen the project and rebuild the same source/Library revision with the same toolchain/key.

These checks are tracked in tickets 68/69 and the project completion backlog. They remain open
until the production pipeline and native journey execute successfully.
