# 46 [D]: In-editor Studio Script editing

**What to build:** Embedded code editor inside the Designer: Tree-sitter-powered highlighting/outline at keystroke latency; on commit, parse-validate against the parser of record — valid hand edits diff into typed commands, invalid ones land line-linked in the problems panel. Canvas edits reflect in text at batch-commit points.

**Blocked by:** 22, 39

**Status:** ready-for-agent

- [ ] Hand-typed valid element appears on canvas immediately after validation
- [ ] Canvas edit appears as a clean canonical diff in the file view
- [ ] Broken syntax isolates to diagnostics without disturbing the live session
- [ ] Comments preserved through round trips
