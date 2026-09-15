# Pi agent chat and daemon relay

## Decision

Studio Builder uses Pi's long-lived RPC mode, not a new `pi -p` process for
each message and not JSON mode.

| Pi interface | Lifecycle | Input | Output | Studio Builder use |
| --- | --- | --- | --- | --- |
| Print (`pi -p`) | One prompt | CLI argument | Final text | Not suitable: loses live process state |
| JSON (`--mode json`) | One invocation | Initial CLI prompt | JSONL events | Useful for batch jobs, not interactive chat |
| RPC (`--mode rpc`) | Long-lived | JSONL commands on stdin | Responses and JSONL events on stdout | Selected: sessions, streaming, tools, queues, abort, model controls |
| TypeScript SDK | In-process | Typed API | Typed events | A future option if the daemon moves to Node/Bun |

References: [Pi RPC protocol](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md),
[Pi JSON event mode](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/json.md),
and [AI Elements conversation patterns](https://elements.ai-sdk.dev/components/conversation).

## Architecture

```text
Browser AgentChat
  ├─ EventSource GET /api/machines/:id/agent
  └─ command POST /api/machines/:id/agent
                    │ authenticated dashboard proxy
                    ▼
             sb-daemon :4545
  ├─ GET  /agent/events     authenticated SSE + cursor replay
  ├─ POST /agent/command    allowlisted Pi RPC commands
  └─ one persistent `pi --mode rpc --continue` child
                    │ strict LF-delimited JSONL
                    ▼
       Pi session in /root/.studio-builder/pi-sessions
```

The browser never receives the daemon bearer token or connects directly to a
builder. The dashboard resolves the active droplet IP server-side, attaches
the per-machine bearer token, and streams the response through the logged-in
dashboard session.

## Implemented feature set

- Persistent context, backed by Pi's durable session and `--continue` after a
  daemon restart.
- Token-by-token assistant text and reasoning updates.
- Structured, collapsible tool calls with live output and error/completion
  states.
- Follow-up queueing when the user sends while Pi is working.
- Stop clears queued work before aborting, matching Pi's documented Escape
  behavior.
- Model and supported thinking-level selectors driven by Pi RPC discovery.
- New conversation without deleting previous Pi session files.
- SSE reconnection, `Last-Event-ID` cursors, a bounded 4,000-event replay
  buffer, and durable message hydration from `get_messages`.
- Pi installation and OpenRouter credentials in new-machine cloud-init.

## Next feature slices

1. Attachments: accept images/files in the composer, cap and validate uploads,
   then map images to Pi's `ImageContent` prompt field and files to safe
   workspace paths.
2. Session browser: expose Pi session listing and let users resume, fork,
   branch, rename, export, and archive conversations.
3. Approval policy: intercept destructive or privileged tools, show an
   explicit confirmation card, and return the decision through Pi extension
   UI RPC.
4. Rich artifacts: recognize changed files, commits, test results, build
   timings, and preview URLs as typed cards rather than relying on fenced
   `ui:*` prose blocks.
5. Reliability: supervise and restart Pi with capped backoff, persist the SSE
   cursor log across daemon restarts, expose health counters, and add a
   dashboard reconnect diagnostic.
6. Multi-agent work: run named Pi sessions as separate workers with a bounded
   concurrency/CPU policy so agent load cannot starve Rust compilation.
7. Console convergence: reuse the authenticated streaming transport for PTY
   sessions while keeping PTY commands separate from the Pi RPC allowlist.

## Deployment note

The chat requires daemon v0.3.0. Build and upload it with
`dashboard/scripts/upload-daemon.ts`, then start a newly provisioned builder
so cloud-init installs the matching binary and Pi environment. Existing live
droplets keep their old daemon until it is replaced and restarted.
