# Define preview and Runtime compilation

Type: grilling
Status: resolved
Blocked by: 02, 03, 04, 06, 07

## Question

How should one Studio Design produce identity-preserving native editor previews and deterministic signed Studio Runtime applications, including validation, Studio IR or AssemblyScript lowering, assets, library snapshots, interactions, diagnostics, and launch verification?

## Answer

One Studio Design produces both outputs through a single validated Runtime Projection. Editor previews reuse the same Runtime catalog and native adapters as package output, preserving stable node identities. No Designer-only rendering behavior; missing protocol properties are explicit catalog enhancements.

Compilation is deterministic and signed: validation, Studio IR or AssemblyScript lowering, assets plus immutable library snapshot, interactions, diagnostics, and launch verification. Manifest asset lists match packaged keys exactly; archives are byte-stable; packages carry signatures verified at launch.

See [CONTEXT.md](../../CONTEXT.md) for the settled glossary.
