# Quickstart: Reload Validation

Run from the repository root with the toolchain environment sourced.

## 1. Scripted session (US1 + US2)

```bash
cargo test -p studio-host --test reload_session
```

Expected: 10 scripted reloads (valid, invalid, crashing) end on the last valid surface;
every outcome acknowledged; no dead window.

## 2. Storm and recovery (US1 + US2)

```bash
cargo test -p studio-host --test reload_storm
```

Expected: 5 requests during one swap collapse to one trailing reload; mid-swap failure
rolls back with disposal cancelled.

## 3. Edit→rebuild→reload e2e (US1)

```bash
cargo test -p studio-cli --test reload_e2e
```

Expected: real CLI client path against a stub host over loopback: build, reload, reject,
restart, cancel with socket removal and exit 130.

## 4. State policy (US4)

Covered inside `reload_session`: preserved route, reset-to-default, fresh instance.

## 5. Manual headless scenario (display-level, gate pattern)

Launch `studio dev <project>` under the headless compositor and edit source or assets. The
CLI owns the native window and commits loose IR/Wasm updates without packaging. Use
`studio restart-session <project>` to request a deliberate state reset. The production
`studio-app` executable accepts only `--bundle <absolute-path>`.

Run `python3 scripts/test-native-hot-reload.py --studio <path-to-studio>` for the native
same-window edit/rejection/recovery smoke test. See `docs/development/HOT_RELOAD.md` for
state transfer and the content-addressed artifact cache. The scripted bundle protocol tests
above remain compatibility coverage.
