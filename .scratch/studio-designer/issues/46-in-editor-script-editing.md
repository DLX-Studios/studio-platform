# 46 [D]: In-editor Studio Script editing

**What to build:** Embedded code editor inside the Designer: Tree-sitter-powered highlighting/outline at keystroke latency; on commit, parse-validate against the parser of record — valid hand edits diff into typed commands, invalid ones land line-linked in the problems panel. Canvas edits reflect in text at batch-commit points.

**Blocked by:** 22, 39

**Status:** done

- [x] Hand-typed valid element appears on canvas immediately after validation — parser-backed insert lowering and session revision are proven at `crates/studio-design/tests/script_editor.rs:111-141`; the mounted editor commit path is `crates/studio-designer/src/focus_view.rs:1742-1827` and `2624-2703`.
- [x] Canvas edit appears as a clean canonical diff in the file view — adapter refresh after a committed `SetProperty` is proven at `crates/studio-design/tests/script_editor.rs:179-212`; the native render path rebases a clean buffer at `crates/studio-designer/src/focus_view.rs:1943-1964`.
- [x] Broken syntax isolates to diagnostics without disturbing the live session — `crates/studio-design/tests/script_editor.rs:144-162` verifies line-linked diagnostics and unchanged revision; native diagnostics are rendered at `crates/studio-designer/src/focus_view.rs:2644-2666`.
- [x] Comments preserved through round trips — `crates/studio-design/tests/script_editor.rs:165-176` verifies trivia transfer; native comment status is exposed at `crates/studio-designer/src/focus_view.rs:2714-2727`.

Tree-sitter's fast tier remains deliberately deferred. The current lexical adapter is dependency-free, parser-backed at commit, and keeps a documented integration seam; the original acceptance boxes above do not make Tree-sitter a separate closure gate.
