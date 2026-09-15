# 44 [D]: Multi-screen navigation and prototype mode

**What to build:** Multiple screens and routes per project; typed navigation interactions between them; prototype mode dispatches declared interactions against isolated ephemeral state that cannot mutate the design except via explicit authoring command.

**Blocked by:** 40

**Status:** done

- [x] Prototype click-through navigates and returns leaving design untouched — isolated dispatch and back navigation are proven at `crates/studio-design/tests/prototype_navigation.rs:134-158`; native enter/return and route/interaction controls are wired at `crates/studio-designer/src/focus_view.rs:1335-1429` and `2218-2242`.
- [x] Interaction graph and event inspector explain behavior without reading generated code — graph entries are rendered with event/action details at `crates/studio-designer/src/focus_view.rs:2604-2622`; graph inspection is covered at `crates/studio-design/tests/prototype_navigation.rs:283-289`.
- [x] Cycles and missing targets produce source-linked diagnostics — `crates/studio-design/tests/prototype_navigation.rs:228-282` verifies interaction IDs and diagnostic codes; runtime diagnostic projection is implemented in `crates/studio-design/src/navigation.rs:221-302`.
