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

## Decisions taken under the closed/deny-by-default reading

| Question from issue 07 | Decision here |
| --- | --- |
| How are renderer kinds reserved? | Structural: the closed schema (`deny_unknown_fields` at every level) exposes no field that registers kinds; composition trees may only reference `ApprovedKindCatalog` entries (snake_case primitive kinds matching `studio-protocol` `NodeKind` serialization). |
| Capability catalog | Reuses the milestone-one closed catalog (`payment.simulate`, `printer.simulate`) shared with bundle manifests. Grows only host-side. |
| Consent scope | Per `(project, plugin, capability)` triple, explicit grant/deny records, revocation deactivates active extensions immediately. |
| Hook budgets | Declared per hook in the descriptor, capped by host ceilings (≤ 5000 ms, ≤ 4 MiB). Violations quarantine the extension; every later hook is refused. |
| Removal safety | `plan_removal` audits owned project artifacts and stores a pending plan without mutation; `complete_removal(force=false)` refuses while artifacts remain, `force=true` treats the report as the pre-mutation disclosure. |

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
- **Primitive-kind spellings.** `DEFAULT_PRIMITIVE_CATALOG` mirrors `NodeKind`
  snake_case serialization at time of writing; a generated catalog from
  `studio-components` should replace it.

## Tests

- Unit: `crates/studio-plugin-registry/src/registry.rs` (admission, tamper, compat,
  unapproved kind, unknown field).
- Integration: `tests/integration/pos_pack_registry.rs` walks the authored first-party
  `pos-pack` fixture through admission → consent → install → lifecycle → removal report,
  plus tamper rejection, expired compatibility, disabled trust keys, time/output budget
  containment, revocation, and closed-schema rejection families.

Authored but not executed here (code-only writer); the serialized runner owns
`cargo fmt/clippy/test --locked`.
