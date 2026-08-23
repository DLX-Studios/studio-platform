# Valid protocol-v1 fixtures

The protocol generator must materialize this complete representative envelope inventory:

- `guest-mount.json`
- `guest-patch.json`
- `guest-navigate.json`
- `guest-action.json`
- `guest-log.json`
- `host-ui.json`
- `host-navigation.json`
- `host-action-result-success.json`
- `host-action-result-failure.json`
- `host-lifecycle.json`

Fixtures are canonical examples shared by Rust contract consumers and the generated AssemblyScript
bindings. Each is pretty-printed deterministically with one trailing newline and contains no
guest-selectable owner field.
