# Implementation Plan: Studio File Routing

**Date**: 2026-11-03 | **Spec**: [spec.md](spec.md)

## Summary and Technical Context

Use the existing Rust/rsvelte frontend, Studio IR, AssemblyScript/Wasm backend and studio-navigation. Preserve the native retained rendering and existing protocol v1 navigation envelopes. No new web framework or external router dependency.

## Architecture

1. Extend routes.rs discovery to .studio and +page naming; layouts never enter route metadata. Delegate concrete matching to studio-navigation, which gains root and wildcard support.
2. Add a bounded project component compiler in studio-script. Resolve local default imports, validate imported names against the same catalog/subset, compose IR with namespaced state/derived/handler/node identities and substitute typed props. Compose ancestor +layout.studio around pages through a default outlet.
3. Compile route pages into a shared module with one component per route. Dev reuses retained screen sessions and the shared host router. Release emits namespaced per-page guest code and a dispatch wrapper, preserving page state across navigation. The host admits navigation commands and validates subsequent remounts against its selected route.
4. Store generated routing metadata in the signed assets under a reserved key. Validate version, page counts, pattern/source identities and bounds before use. Legacy packages without it keep current behavior.
5. Integrate project checks, file watching, source identities and build discovery. Source remains .studio; generated AssemblyScript stays under build/.

## Constitution Check

Host owns resolution/history/admission. Existing navigation envelopes remain closed. New metadata has versioned limits and negative tests. No guest timer/native import or secret authority is added. Red-green tests precede each behavior slice. Integration checks use one Cargo process, consistent debug=0/incremental=0, format/tests/Clippy sequentially. No release build is requested. Existing unrelated dirty files remain intact.

## Slices and cross-cutting justification

Navigation, compiler composition and runtime integration are independent foundations with a shared contract. The integration slice touches more than five files because dev and Wasm must implement the same behavior atomically rather than claiming one-path support. Parallel work uses separate source boundaries; Cargo verification stays serialized by the root agent.

## Source Structure

crates/studio-navigation/src/{route,tree,project}.rs; crates/studio-cli/src/{routes,studio_project,dev/compiler,dev/surface,build/mod,main}.rs; crates/studio-script/src/{project,validate,assemblyscript}.rs; crates/studio-runtime/src/plugin_surface.rs; examples/stream-countdown/routes/{+layout,+page}.studio and routes/live/+page.studio; components/*.studio.

## Automatic countdown slice

A source-only Timer compiles into invisible native nodes: one status text and ordinary pressed handlers for tick/completion. Compiler-generated versioned timer metadata uses a separate signed reserved asset. A shared pure scheduler in studio-navigation owns deadlines; dev and production hosts read completion status and dispatch at most 32 due events per poll through existing bounded guest calls. Timers cancel on route exit and resume retained page state on return. A 700 ms completion delay allows the clock opacity transition to finish. No protocol variant, native catalog kind or Wasmtime import is added. General changed native props must be patched by the evaluator so opacity follows state.

## Native presentation slice

Keep the retained component catalog and add closed presentation props to its existing property contract: Box.background_asset uses an admitted image behind content with cover; Text.font_size is bounded to 8–256 logical pixels and scales with preview zoom; Text.color accepts exactly six hex digits. Native opacity tracks sample the host clock, retain node identity, handle interruption, and snap under reduced motion. The example uses original local sunset art and fixed outlined Pricedown title art; font software is not bundled. Native metrics, cover bounds, foreground input, and timed opacity tests verify the rendering path separately from compiler tests.

## Verification

Focused router, scanner, component composition, dev navigation and compiled-Wasm tests first. Then formatting, workspace tests and Clippy sequentially. Record environmental and pre-existing failures separately. Run source checks and a bounded native preview when available. Do not run release-profile builds.
