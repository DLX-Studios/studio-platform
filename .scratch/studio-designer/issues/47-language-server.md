# 47 [D]: First-party language server (LSP)

**What to build:** Independent LSP artifact layering Tree-sitter fast responses over parser-of-record semantics: completion for component kinds, token names, plugin SDK surfaces, and `$item` fields typed from declared response schemas; hover types; go-to-definition across project and plugin code. Functions headlessly for any editor and never requires a running Designer.

**Blocked by:** 22, 35

**Status:** ready-for-agent

- [ ] Server operates over stdio in VS Code/Zed/Neovim with diagnostics and completion
- [ ] Completion includes schema-derived `$item` fields for bound collections
- [ ] No dependency on Designer process or its storage
- [ ] Integration test drives server programmatically end to end
