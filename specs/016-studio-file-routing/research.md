# Research

- Decision: reuse studio-navigation, the intentionally Studio-owned router inspired by gpui-router/gpui-nav. The current CLI matcher duplicates it and root/catch-all support is missing in the shared tree. Directly adopting an upstream router would not integrate the existing bounded stack and guards.
- Decision: +page.studio / +layout.studio follow familiar SvelteKit authoring; retain existing index/named page compatibility. Layouts use a default outlet. Server-side/browser features remain outside this native host.
- Decision: compile imported component instances into scoped IR, keeping the evaluator and Wasm backend aligned; do not evaluate arbitrary TypeScript or add a web runtime.
- Decision: signed project route metadata lets the native host validate page selection; guest navigation requests use existing protocol v1 commands and host navigation events.
