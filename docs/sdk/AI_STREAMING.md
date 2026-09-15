# AI streaming contract

The AI SDK sends both completion modes through the host REST broker. A streaming request is a
bounded `POST /v1/chat/completions/stream` with the canonical JSON body, including ordered
`messages`, `temperature`, and optional `stream_options.include_usage`; the host validates the
body before credential injection or network dispatch. Reconnects resend the exact body and carry
the last received SSE event id.

The host SSE adapter preserves event order. Validated delta chunks are followed by an optional
typed usage event (`choices: []` with `usage`) and then a completed event. Provider error events
are surfaced as typed AI errors. The OpenAI `[DONE]` sentinel is treated as clean completion.

Cancellation is host-owned and cooperative: it stops delivery between reads, prevents further
reconnects, and emits one terminal cancellation event. Request and stream bounds remain enforced
across reconnects, and malformed or oversized data fails before guest visibility.

The staging fixture for this contract is deterministic and transport-free: the `studio-net`
streaming tests record the canonical POST body, split SSE frames across reads, exercise usage/error
events, and verify cancellation, reconnect, event ordering, and cumulative bounds.
