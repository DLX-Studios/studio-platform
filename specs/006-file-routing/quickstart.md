# Quickstart: Routing Validation

## 1. Registry matrix and golden

```bash
cargo test -p studio-cli --test route_registry
```

Expected: every file shape maps correctly; duplicates, ambiguity, and all malformed
families rejected with named diagnostics; generated module matches the golden
byte-for-byte.

## 2. SDK battery

```bash
bun test sdk/assemblyscript/tests/navigation-routes.test.ts
```

Expected: the 40-path battery resolves identically to the Rust matcher.

## 3. Reload policy patterns

Covered in `studio-app` reload unit tests: parameterized current routes preserved by
pattern; removed routes reset.

## 4. Full gate (deferred)

Per the post-008 verification phase: fmt, clippy, workspace tests, Bun suites.
