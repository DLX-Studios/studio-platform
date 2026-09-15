# Quickstart: Developer Experience Validation

## 1. Server protocol

```bash
cargo test -p studio-language-server
```

Expected: stdio round-trips plus format goldens, diagnostic fixtures, projection
mappings, and failure isolation all green.

## 2. Client packaging (manual)

Open `editors/vscode` in VS Code, launch the extension host against a fixture
project, and verify activation, highlighting, diagnostics, commands, and degraded
mode with the server binary renamed away.

## 3. Full gate (deferred)

Per the post-008 verification phase.
