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

Launch `studio-app --dev <bundle> --reload-socket <path>` under the headless compositor,
deliver a reload request via the socket, and observe the same-window swap. Record evidence
per the release checklist pattern.
