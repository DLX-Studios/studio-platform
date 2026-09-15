# Quickstart: Studio Toolchain Validation

Validates the toolchain feature end-to-end. Run from the repository root after
`bun install --frozen-lockfile` (or the documented filesystem-safe equivalent) and with the
pinned Bun 1.3.9 on PATH. See [contracts/cli.md](contracts/cli.md) for the command contract.

## 1. Clean build (US1)

```bash
cargo build --locked -p studio-cli
target/debug/studio build starter
```

Expected: exit `0`; `examples/starter/build/starter.studio` exists.

## 2. Repeated builds are byte-identical (SC-002)

```bash
target/debug/studio build starter
cp examples/starter/build/starter.studio /tmp/first.studio
target/debug/studio build starter
cmp examples/starter/build/starter.studio /tmp/first.studio
```

Expected: `cmp` reports no difference.

## 3. Failed build is structured and non-destructive (US1, US4)

```bash
cp -r examples/starter /tmp/studio-broken-example   # fixture with a broken source
# introduce an asc-level error into /tmp/studio-broken-example/assembly/index.ts
target/debug/studio build /tmp/studio-broken-example ; echo "exit=$?"
```

Expected: exit `11`; stderr contains a JSON diagnostic with `"phase":"compile"` and a stable
code; any previously built bundle is untouched and no partial bundle exists.

## 4. Watch loop hygiene (US2)

```bash
target/debug/studio dev starter
# in another terminal: touch examples/starter/assembly/routes.generated.ts
#  → no rebuild (generated churn excluded)
# save a real source file five times quickly → exactly one rebuild after the settle window
# Ctrl-C the dev session → clean stop, exit 130, no partial bundle
```

Automated coverage for the same behaviors: `cargo test -p studio-cli --test watch_session`.

## 5. Shim parity (US3)

```bash
bun run build:starter
target/debug/studio build starter
cmp examples/starter/build/starter.studio /tmp/first.studio
```

Expected: the documented Bun entry and the native command produce the identical bundle and the
same exit codes.

## 6. Automated suite

```bash
cargo test --locked -p studio-cli
bun run build:starter && bun test tests/e2e/starter_plugin.test.ts
./scripts/test-starter-quickstart.sh
```

Expected: all pass; quickstart stays under its ten-minute ceiling.
