//! Server-sent-event streaming engine with validated typed chunks, cancellation, bounds, and a
//! host-owned reconnect/retry policy.
//!
//! The broker owns every reconnect decision: guests observe [`StreamEvent`]s only and never
//! learn transport addresses, retry internals beyond scheduled delays, or unvalidated payloads.
//! Cancellation is cooperative and takes effect between chunk reads; bounds cover total stream
//! bytes, validated chunk count, single-frame size, and wall-clock lifetime.

use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use studio_security::SensitiveValueFilter;

use crate::declaration::CompiledRouteGroup;
use crate::error::{BrokerError, BrokerErrorCode};
use crate::guest::StreamEvent;
use crate::limits::EffectiveLimits;
use crate::schema::JsonSchema;
use crate::transport::{HttpTransport, OutgoingRequest, TransportError};

/// Maximum bytes for one buffered server-sent-event frame before it is rejected.
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Cooperative cancellation shared between the guest handle and the pump thread.
#[derive(Clone, Default)]
pub(crate) struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Shared event queue between the pump thread and the guest handle.
pub(crate) struct StreamChannel {
    state: Mutex<ChannelState>,
    signal: Condvar,
    token: CancellationToken,
}

struct ChannelState {
    events: VecDeque<StreamEvent>,
    closed: bool,
}

impl StreamChannel {
    pub(crate) fn new(token: CancellationToken) -> Self {
        Self {
            state: Mutex::new(ChannelState {
                events: VecDeque::new(),
                closed: false,
            }),
            signal: Condvar::new(),
            token,
        }
    }

    pub(crate) fn push(&self, event: StreamEvent) {
        let mut state = self.lock();
        state.events.push_back(event);
        drop(state);
        self.signal.notify_all();
    }

    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        drop(state);
        self.signal.notify_all();
    }

    /// Next typed event; blocks until one arrives or the stream closes and drains.
    #[must_use]
    pub fn next_event(&self) -> Option<StreamEvent> {
        let mut state = self.lock();
        loop {
            if let Some(event) = state.events.pop_front() {
                return Some(event);
            }
            if state.closed {
                return None;
            }
            state = self
                .signal
                .wait_timeout(state, Duration::from_millis(250))
                .map_or_else(|poison| poison.into_inner().0, |(guard, _result)| guard);
        }
    }

    /// Request host-side cancellation of the underlying stream.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ChannelState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Spawn the host-owned pump thread for one admitted streaming route.
pub(crate) fn spawn_stream(
    transport: Arc<dyn HttpTransport>,
    group: CompiledRouteGroup,
    outgoing: OutgoingRequest,
    filter: Arc<Mutex<SensitiveValueFilter>>,
) -> crate::guest::StreamHandle {
    let token = CancellationToken::default();
    let channel = Arc::new(StreamChannel::new(token.clone()));
    let worker_channel = Arc::clone(&channel);
    let limits = *group.limits();
    let policy = group.retry_policy();
    let channel_for_handle = Arc::clone(&channel);
    std::thread::Builder::new()
        .name("studio-net-stream".to_owned())
        .spawn(move || {
            let pump_channel = Arc::clone(&worker_channel);
            let close_channel = Arc::clone(&worker_channel);
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                pump(
                    transport,
                    group,
                    outgoing,
                    pump_channel,
                    token,
                    filter,
                    limits,
                    policy,
                );
            }));
            close_channel.close();
        })
        .map(|_join| crate::guest::StreamHandle::new(channel_for_handle))
        .unwrap_or_else(|_| {
            // Thread spawn failure fails closed: no stream exists, so no events flow.
            channel.push(StreamEvent::Failed(BrokerError::new(
                BrokerErrorCode::TransportFailure,
            )));
            channel.close();
            crate::guest::StreamHandle::new(channel)
        })
}

#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "the pump interleaves bounds, cancellation, framing, and reconnect policy in one auditable loop"
)]
fn pump(
    transport: Arc<dyn HttpTransport>,
    group: CompiledRouteGroup,
    mut outgoing: OutgoingRequest,
    channel: Arc<StreamChannel>,
    token: CancellationToken,
    filter: Arc<Mutex<SensitiveValueFilter>>,
    limits: EffectiveLimits,
    policy: crate::declaration::RetryPolicy,
) {
    let Some(chunk_schema) = group.chunk_schema() else {
        return;
    };
    let started = Instant::now();
    let mut attempt: u32 = 0;
    let mut backoff = policy.base_delay;
    let mut last_event_id: Option<String> = None;
    let mut total_bytes: usize = 0;
    let mut events: u64 = 0;
    let mut opened = false;
    loop {
        if channel_exhausted(&channel, &token, started, limits.stream_max_duration) {
            return;
        }
        if let Some(id) = &last_event_id {
            set_header(&mut outgoing.headers, "last-event-id", id.clone());
        }
        match transport.open_stream(clone_outgoing(&outgoing)) {
            Err(error) => {
                if !retry_or_fail(
                    &channel,
                    &token,
                    &policy,
                    &mut attempt,
                    &mut backoff,
                    &error,
                    started,
                    limits.stream_max_duration,
                ) {
                    return;
                }
            }
            Ok(mut stream) => {
                if !opened {
                    opened = true;
                    channel.push(StreamEvent::Opened);
                }
                match read_connection(
                    &mut *stream,
                    &channel,
                    &token,
                    chunk_schema,
                    &mut last_event_id,
                    &mut total_bytes,
                    &mut events,
                    started,
                    &limits,
                    &filter,
                ) {
                    ConnectionEnd::Completed | ConnectionEnd::Stop => return,
                    ConnectionEnd::Retryable(error) => {
                        if !retry_or_fail(
                            &channel,
                            &token,
                            &policy,
                            &mut attempt,
                            &mut backoff,
                            &error,
                            started,
                            limits.stream_max_duration,
                        ) {
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn channel_exhausted(
    channel: &StreamChannel,
    token: &CancellationToken,
    started: Instant,
    max_duration: Duration,
) -> bool {
    if token.is_cancelled() {
        channel.push(StreamEvent::Cancelled);
        return true;
    }
    if started.elapsed() >= max_duration {
        channel.push(StreamEvent::Failed(BrokerError::new(
            BrokerErrorCode::Timeout,
        )));
        return true;
    }
    false
}

/// Decide whether another host-owned reconnect fits the policy; emit the terminal failure
/// otherwise. Returns `false` when the pump must stop.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn retry_or_fail(
    channel: &StreamChannel,
    token: &CancellationToken,
    policy: &crate::declaration::RetryPolicy,
    attempt: &mut u32,
    backoff: &mut Duration,
    error: &TransportError,
    started: Instant,
    max_duration: Duration,
) -> bool {
    if *attempt < policy.max_reconnects {
        let remaining = max_duration.saturating_sub(started.elapsed());
        let delay = (*backoff).min(remaining);
        if !delay.is_zero() {
            *attempt += 1;
            channel.push(StreamEvent::RetryScheduled {
                attempt: *attempt,
                delay_ms: delay.as_millis().try_into().unwrap_or(u64::MAX),
            });
            sleep_cancellable(token, delay);
            *backoff = (*backoff).saturating_mul(2);
            return true;
        }
    }
    channel.push(StreamEvent::Failed(map_transport_error(*error)));
    false
}

enum ConnectionEnd {
    Completed,
    Stop,
    Retryable(TransportError),
}

#[allow(clippy::too_many_arguments)]
fn read_connection(
    stream: &mut dyn crate::transport::ByteStream,
    channel: &Arc<StreamChannel>,
    token: &CancellationToken,
    chunk_schema: &JsonSchema,
    last_event_id: &mut Option<String>,
    total_bytes: &mut usize,
    events: &mut u64,
    started: Instant,
    limits: &EffectiveLimits,
    filter: &Arc<Mutex<SensitiveValueFilter>>,
) -> ConnectionEnd {
    let mut parser = SseParser::new(MAX_FRAME_BYTES);
    loop {
        if token.is_cancelled() {
            channel.push(StreamEvent::Cancelled);
            return ConnectionEnd::Stop;
        }
        if started.elapsed() >= limits.stream_max_duration {
            channel.push(StreamEvent::Failed(BrokerError::new(
                BrokerErrorCode::Timeout,
            )));
            return ConnectionEnd::Stop;
        }
        match stream.read_chunk() {
            Err(error) => return ConnectionEnd::Retryable(error),
            Ok(None) => {
                channel.push(StreamEvent::Completed);
                return ConnectionEnd::Completed;
            }
            Ok(Some(bytes)) => {
                *total_bytes = (*total_bytes).saturating_add(bytes.len());
                if *total_bytes > limits.max_stream_bytes {
                    channel.push(StreamEvent::Failed(BrokerError::with_detail(
                        BrokerErrorCode::StreamExceeded,
                        "stream byte budget exhausted".to_owned(),
                    )));
                    return ConnectionEnd::Stop;
                }
                if parser.push(&bytes).is_err() {
                    channel.push(StreamEvent::Failed(BrokerError::with_detail(
                        BrokerErrorCode::StreamExceeded,
                        "single frame exceeded size bound".to_owned(),
                    )));
                    return ConnectionEnd::Stop;
                }
                let frames = match parser.drain_frames() {
                    Ok(frames) => frames,
                    Err(()) => {
                        channel.push(StreamEvent::Failed(BrokerError::with_detail(
                            BrokerErrorCode::StreamExceeded,
                            "single frame exceeded size bound".to_owned(),
                        )));
                        return ConnectionEnd::Stop;
                    }
                };
                for frame in frames {
                    let Some(parsed) = parse_sse_frame(&frame) else {
                        continue;
                    };
                    if let Some(id) = parsed.id {
                        *last_event_id = Some(id);
                    }
                    if parsed.data.is_empty() {
                        continue;
                    }
                    if parsed.data == "[DONE]" {
                        channel.push(StreamEvent::Completed);
                        return ConnectionEnd::Completed;
                    }
                    *events += 1;
                    if *events > limits.max_stream_events {
                        channel.push(StreamEvent::Failed(BrokerError::with_detail(
                            BrokerErrorCode::StreamExceeded,
                            "chunk event budget exhausted".to_owned(),
                        )));
                        return ConnectionEnd::Stop;
                    }
                    match deliver_chunk(chunk_schema, &parsed.data) {
                        Ok(value) => {
                            // Chunks echoing registered credential material are dropped and
                            // terminate the stream before guest visibility.
                            if chunk_contains_registered_material(&value, filter) {
                                channel.push(StreamEvent::Failed(BrokerError::with_detail(
                                    BrokerErrorCode::SensitiveContentRejected,
                                    String::from("chunk contained protected material"),
                                )));
                                return ConnectionEnd::Stop;
                            }
                            channel.push(StreamEvent::Chunk(value));
                        }
                        Err(code) => {
                            let detail = sanitize_detail(filter, code.detail_text());
                            channel.push(StreamEvent::Failed(BrokerError::with_detail(
                                code.code_enum(),
                                detail,
                            )));
                            return ConnectionEnd::Stop;
                        }
                    }
                }
            }
        }
    }
}

fn deliver_chunk(schema: &JsonSchema, raw: &str) -> Result<Value, ChunkFailure> {
    let value: Value = serde_json::from_str(raw).map_err(|_| ChunkFailure::Malformed)?;
    schema
        .validate(&value)
        .map_err(|_| ChunkFailure::Mismatch)?;
    Ok(value)
}

fn chunk_contains_registered_material(
    value: &Value,
    filter: &Arc<Mutex<SensitiveValueFilter>>,
) -> bool {
    serde_json::to_string(value)
        .map(|rendered| {
            filter
                .lock()
                .map(|filter| filter.validate_persistence(&rendered).is_err())
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

#[derive(Clone, Copy)]
enum ChunkFailure {
    Malformed,
    Mismatch,
}

impl ChunkFailure {
    fn code_enum(self) -> BrokerErrorCode {
        match self {
            Self::Malformed => BrokerErrorCode::ResponseMalformed,
            Self::Mismatch => BrokerErrorCode::ResponseSchemaMismatch,
        }
    }

    const fn detail_text(self) -> &'static str {
        match self {
            Self::Malformed => "chunk was not valid JSON",
            Self::Mismatch => "chunk failed declared schema",
        }
    }
}

fn sanitize_detail(filter: &Arc<Mutex<SensitiveValueFilter>>, text: &str) -> String {
    filter
        .lock()
        .map(|filter| filter.sanitize(text))
        .unwrap_or_else(|poison| poison.into_inner().sanitize(text))
}

fn map_transport_error(error: TransportError) -> BrokerError {
    match error {
        TransportError::TimedOut => BrokerError::new(BrokerErrorCode::Timeout),
        TransportError::ConnectionFailure | TransportError::BodyTooLarge => {
            BrokerError::new(BrokerErrorCode::TransportFailure)
        }
    }
}

fn sleep_cancellable(token: &CancellationToken, mut remaining: Duration) {
    while !remaining.is_zero() && !token.is_cancelled() {
        let step = remaining.min(Duration::from_millis(25));
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}

fn clone_outgoing(request: &OutgoingRequest) -> OutgoingRequest {
    request.clone()
}

fn set_header(headers: &mut Vec<(String, String)>, name: &str, value: String) {
    for (existing_name, existing_value) in headers.iter_mut() {
        if existing_name == name {
            *existing_value = value;
            return;
        }
    }
    headers.push((name.to_owned(), value));
}

struct SseFrame {
    data: String,
    id: Option<String>,
}

fn parse_sse_frame(frame: &[u8]) -> Option<SseFrame> {
    let text = std::str::from_utf8(frame).ok()?;
    let mut data_lines: Vec<&str> = Vec::new();
    let mut id: Option<String> = None;
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "data" => data_lines.push(value),
            "id" if !value.is_empty()
                && value.len() <= 256
                && value.bytes().all(|byte| byte.is_ascii_graphic()) =>
            {
                id = Some(value.to_owned());
            }
            "id" => {}
            // `event:` typing is declared host-side; server `retry:` hints never override the
            // signed host-owned reconnect policy.
            _ => {}
        }
    }
    if data_lines.is_empty() && id.is_none() {
        return None;
    }
    Some(SseFrame {
        data: data_lines.join("\n"),
        id,
    })
}

/// Incremental byte-level SSE framer splitting complete events on blank lines.
struct SseParser {
    buffer: Vec<u8>,
    max_frame_bytes: usize,
}

impl SseParser {
    fn new(max_frame_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_frame_bytes,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() > self.max_frame_bytes.saturating_mul(2) {
            return Err(());
        }
        Ok(())
    }

    fn drain_frames(&mut self) -> Result<Vec<Vec<u8>>, ()> {
        let mut frames = Vec::new();
        while let Some((end, separator)) = find_frame_boundary(&self.buffer) {
            if end > self.max_frame_bytes {
                return Err(());
            }
            let frame: Vec<u8> = self.buffer.drain(..end).collect();
            self.buffer.drain(..separator);
            frames.push(frame);
        }
        Ok(frames)
    }
}

fn find_frame_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    if buffer.len() < 2 {
        return None;
    }
    for index in 0..=(buffer.len() - 2) {
        if buffer[index] == b'\r'
            && buffer.len() >= index + 4
            && &buffer[index..index + 4] == b"\r\n\r\n"
        {
            return Some((index, 4));
        }
        if buffer[index] == b'\n' && buffer[index + 1] == b'\n' {
            return Some((index, 2));
        }
    }
    None
}
