# CLI tasks and native windows

All tools operate in the canonical folder returned by `project.info`.

| Tool | Behavior |
| --- | --- |
| `preview.start` | Start `studio dev <root>`, or report an existing responding preview. |
| `preview.status` | Query live revision, phase, diagnostic, selection, and presentation over the existing dev RPC. |
| `preview.logs` | Read process state and stdout/stderr of the preview this MCP session started. |
| `preview.stop` | Stop this session's preview child. Does not stop an external preview. |
| `preview.restart` | Start `studio restart-session <root>` to reset guest state; tracked as a CLI task. |
| `studio.check` | Start `studio check` for permitted `.studio` source files, excluding bundles/generated files. |
| `studio.build` | Start `studio build <root>` to produce a release archive. |
| `studio.task.status` | Poll the latest check/build/restart command's completion and exit code. |
| `studio.task.logs` | Read that command's stdout and stderr, including compiler diagnostics. |
| `studio.task.cancel` | Stop that command. |
| `runtime.start` | Start `studio-app --bundle <path>` with `{"bundle": "build/my-app.studio"}`. |
| `runtime.status` | Poll that release process's running state and exit code. |
| `runtime.logs` | Read that release process's stdout/stderr. |
| `runtime.stop` | Stop that release process. |

Check, build, and restart share one CLI task slot. Starting a task while one
runs returns `PROCESS_BUSY`; poll or deliberately cancel before replacing it.
Preview and runtime each have a separate process slot. Every launch returns
promptly: a running process is not proof of successful compilation, bundle
admission, or a rendered window. Poll with a bounded deadline appropriate to
the task; on failure, read logs and fix the cause before retrying.

Process status reports `phase` (`not_started`, `running`, `succeeded`, `failed`,
or `stopped`), `running`, and for managed processes `pid`, `exit_code`,
`output_complete`, `command`, and `args`. After exit, wait for
`output_complete: true` to read the final diagnostics. Each log stream retains
the last 64 KiB; `discarded_bytes` reports truncation. Starting a new process
in a completed slot replaces the previous process's logs.

`preview.status` is live dev RPC state, while `preview.logs` is process state
for this MCP session's child. A preview started elsewhere can answer status
and presentation requests while having no logs owned by this session.

The MCP server owns its children and stops them on normal session closure.
Unix cancellation kills the child's process group, including compiler
children; other platforms terminate the immediate child. Closing the agent
session therefore closes windows launched through its MCP tools. To keep a
window independent of the agent, start the CLI/runtime yourself.

Dev watches loose source files and reloads hashed IR/Wasm without packaging
on each edit. Use `studio.build` only when a release bundle is needed.
Build currently follows the CLI's existing single-entry/AssemblyScript
packaging pipeline; multi-screen dev support does not imply multi-screen
release packaging. A successful dev preview also does not provision publisher
trust for a release bundle.

Runtime bundle paths must be normalized project-relative `.studio` files;
symlinks resolving outside the project are denied. The release executable
keeps its existing signature/trust checks. Runtime tools manage its process
only: dev selection, hot reload, screen controls, and inspection use the dev
host. They do not add an MCP/debug server to the release executable.
