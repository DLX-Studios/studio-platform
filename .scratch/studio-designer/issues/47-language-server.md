# 47 [D]: First-party language server (LSP)

**What to build:** Independent LSP artifact layering Tree-sitter fast responses over parser-of-record semantics: completion for component kinds, token names, plugin SDK surfaces, and `$item` fields typed from declared response schemas; hover types; go-to-definition across project and plugin code. Functions headlessly for any editor and never requires a running Designer.

**Blocked by:** 22, 35

**Status:** closed

- [x] Server operates over stdio in VS Code/Zed/Neovim with diagnostics and completion — framed JSON-RPC handling and the initialize, didOpen, didChange, diagnostic, completion, hover, definition, and shutdown sequence are exercised in `crates/studio-language-server/tests/stdio_roundtrip.rs:38-158`; the server loop is `crates/studio-language-server/src/lib.rs:532-620`.
- [x] Completion includes schema-derived `$item` fields for bound collections — declared schemas are completed in `crates/studio-language-server/src/lib.rs:409-420`; plugin manifest `routes[].responseSchema` fields are indexed at `crates/studio-language-server/src/lib.rs:919-969` and covered by `crates/studio-language-server/tests/stdio_roundtrip.rs:160-199`.
- [x] No dependency on Designer process or its storage — `crates/studio-language-server/Cargo.toml:9-12` depends only on serde, serde_json, and studio-script; `Workspace` is in-memory/source-backed (`crates/studio-language-server/src/lib.rs:126-322`).
- [x] Integration test drives server programmatically end to end — `crates/studio-language-server/tests/stdio_roundtrip.rs:38-158` drives framed requests through `LanguageServer::serve` and asserts responses/diagnostics.

## Closure audit (2026-08-29)

`cargo test --locked -p studio-language-server` passes, including
manifest-backed `$item` schema completion and definition lookup. The artifact
is standalone and can be launched through the `studio-language-server` binary
in `crates/studio-language-server/src/bin/studio-language-server.rs:1-11`.
