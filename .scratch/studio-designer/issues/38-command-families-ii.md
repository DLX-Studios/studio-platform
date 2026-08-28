# 38 [D]: Command families II — responsive, tokens, bindings, interactions, compositions

**What to build:** Remaining command families through the same engine: base-plus-breakpoint responsive values, design-token definition/application, typed content bindings, declarative interactions graph, and Reusable Composition define/instance/override semantics including identity-retaining propagation from definition updates.

**Blocked by:** 37

**Status:** done

- [x] Each family demoable through public seam queries and CLI replay
- [x] Composition instance reflects definition change while retaining instance identity
- [x] Overrides admitted only where the composition contract allows
- [x] Stale-precondition submissions return structured conflicts, never silent overwrite
- [x] Unknown fields in any closed schema are rejected at decode

## Verification evidence

Verified on the Sprint 03 current tip with the focused, commit-specific target
cache and the required low-footprint Cargo settings:

- `cargo test --locked -p studio-design --test designer_session_seam`: 10 passed,
  including public command-family, composition identity, override rejection,
  stale/precondition conflict, persistence/reopen, and nested closed-schema
  assertions.
- `cargo test --locked -p studio-design --test content_collections`: 5 passed.
- `cargo test --locked -p studio-design --test tokens`: 2 passed.
- `cargo test --locked -p studio-design --test responsive_profiles`: 4 passed.
- `cargo test --locked -p studio-design --test prototype_navigation`: 3 passed.
- `cargo test --locked -p studio-cli --test replay`: 1 passed, covering typed
  CLI replay, deterministic repeated input, durable reopen equality, accepted
  receipts, and structured forbidden-override rejection.

The CLI replay path is `studio replay <json-file>` (or stdin) and accepts a
closed `{ design, batches }` envelope. It executes through `DesignerSession`
and reports outcomes, current/reopened snapshots, and a deterministic flag.
