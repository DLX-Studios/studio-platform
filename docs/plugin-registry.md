# Plugin / extension registry foundation

Implementation notes for ticket 35 (`Plugin/extension registry foundation`). Refines open
grilling issue 07 (`07-define-extension-authority.md`); descriptor scope may adjust when
that issue resolves.

## Crate layout

- `crates/studio-plugin-registry` — closed descriptor schema v1, signed-envelope
  verification, per-project consent ledger, bounded lifecycle runner, structural
  contribution-kind approval, removal reporting.
- `crates/studio-package` gained `sign_document` / `verify_document_signature` /
  `canonical_document_bytes`: standalone document-domain signatures
  (`studio.document.signature.v1`) that reuse the existing provisioned
  [`TrustStore`](../../crates/studio-package/src/trust.rs) and Ed25519 verification path
  used for bundle signatures. Only the domain separation differs.

## Alignment with ticket 18 (protected secret store)

Branch merges `origin/tt/18-protected-secret-store`. Descriptor secret references follow
the landed conventions exactly: `SettingsFieldType::SecretReference { name, purpose }`
mirrors manifest `SecretDeclaration` and `ProtectedSecretKey` — `name` is a stable
lowercase identifier (`[a-z0-9._-]`, ≤ 128, lowercase first character), `purpose` is safe
host-visible text (≤ 256), and names are unique per descriptor. Designer-configured values
therefore resolve against the same app/environment-scoped protected partitions; plaintext
never appears in descriptors, consent records, or usage ledgers.

## Decisions taken under the closed/deny-by-default reading

| Question from issue 07 | Decision here |
| --- | --- |
| How are renderer kinds reserved? | Structural: the closed schema (`deny_unknown_fields` at every level) exposes no field that registers kinds; composition trees may only reference host-supplied `ApprovedKindCatalog` entries. The shipped default is a conservative subset of snake_case `studio-protocol` `NodeKind` values. |
| Integrity shape | Schema v1 fixes JCS canonicalization, the `studio.document.signature.v1` domain, and Ed25519. The envelope carries publisher/key attribution plus the signature; admission retains the verified canonical document digest for audit. |
| Capability catalog | Reuses the milestone-one closed catalog (`payment.simulate`, `printer.simulate`) shared with bundle manifests. Grows only host-side. |
| Consent scope | Per `(project, plugin, capability)` triple, inspectable grant/deny records; revocation or an explicit denial deactivates active extensions immediately. |
| Hook budgets | Declared per hook in the descriptor, capped by host ceilings (≤ 5000 ms, ≤ 4 MiB). The admission hook runs only after signature, compatibility, descriptor, and kind validation. Later violations quarantine the extension; every further hook is refused. |
| Removal safety | `plan_removal` audits owned project artifacts and stores a pending plan without mutation; any usage change makes the report stale. `complete_removal(force=false)` refuses while artifacts remain, while `force=true` only proceeds against a still-current pre-mutation disclosure. |

## Underspecified points flagged for issue 07

1. **Capability namespace.** Designer-side capabilities (settings writes, route groups,
   content collections) have no catalog yet; only the runtime simulator pair exists.
2. **Content Collection types, validators, migrations** are named in the spec as
   contribution surfaces but have no data shape anywhere; the schema reserves room in
   `Contributions` but does not guess one.
3. **Merge rules** for overlapping contributions across multiple enabled plugins are a
   build-time concern (spec § Integration plugins) and intentionally out of scope here.
4. **Command execution** goes through `DesignerSession`; this registry only validates that
   commands reference declared declarative actions.

## UNVERIFIED

- **Real memory enforcement.** Hook memory budgets gate declared ceilings and output size
  deterministically; actual linear-memory preemption belongs to the wasm runtime host
  (fuel/epoch interruption). Time containment likewise detects overruns after return
  rather than preempting synchronous handlers.
- **Wire compatibility of signature formats.** Descriptor signatures use a new document
  domain rather than `CanonicalBundleInput`; whether future packaging folds descriptors
  into bundle manifests is unresolved until issue 07 lands.
- **Primitive-kind coverage.** `DEFAULT_PRIMITIVE_CATALOG` is a hand-maintained conservative
  subset of `NodeKind` snake_case serialization. A generated readiness-aware catalog from
  `studio-components` should replace it.

## Tests

- Unit: `crates/studio-plugin-registry/src/registry.rs` (admission, tamper, compat,
  unapproved kind, unknown field).
- Integration: `tests/integration/pos_pack_registry.rs` walks the authored first-party
  `pos-pack` fixture through admission → consent → install → lifecycle → removal report,
  plus tamper rejection, expired compatibility, disabled trust keys, time/output budget
  containment, guest-trap containment, explicit denial/revocation, stale-removal protection,
  and closed-schema rejection families.

Authored but not executed here (code-only writer); the serialized runner owns
`cargo fmt/clippy/test --locked`.
