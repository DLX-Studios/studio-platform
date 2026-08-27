#![allow(clippy::pedantic)]
#![allow(clippy::too_many_lines)]
//! Host-owned REST request broker governed by signed route-group declarations.
//!
//! Applications never open sockets or issue raw HTTP. All outbound REST traffic flows through
//! this broker, which admits requests only against signed route-group declarations constraining
//! origins, methods, paths, headers, and request/response JSON schemas, with explicit generous
//! size, rate, timeout, and streaming bounds. Credentials resolve per route group from public
//! access, an OAuth provider-plugin session seam (filled by the integration-plugin milestone),
//! or a named protected secret injected by [`studio_security::BrokerSecretInjector`] strictly at
//! send time. Response bodies are validated against the declared schema before any guest
//! visibility, and every diagnostic passes through the redaction scrubber.
//!
//! Module map:
//!
//! - [`declaration`]: signed route-group declaration schema and validation.
//! - [`limits`]: host ceilings and declaration defaults for sizes, rates, timeouts, streams.
//! - [`schema`]: bounded closed JSON Schema subset validator used for declared shapes.
//! - [`transport`]: host transport abstraction; guests never see these types.
//! - [`admission`]: request-to-route-group matching with stable denial codes.
//! - [`credential`]: per-group credential resolution including the OAuth session hook.
//! - [`broker`]: execution pipeline: admit, bound, inject at send time, validate pre-guest.
//! - [`streaming`]: server-sent-event mode with typed validated chunks, cancellation, and a
//!   host-owned reconnect/retry policy.
//! - [`error`]: stable value-free broker error codes safe for guest surfaces.
//! - [`guest`]: the only surface exposed to guest code: handles plus typed events.

pub mod admission;
pub mod broker;
pub mod credential;
pub mod declaration;
pub mod error;
pub mod guest;
pub mod limits;
pub mod schema;
pub mod streaming;
pub mod transport;

pub use error::{BrokerError, BrokerErrorCode};
pub use guest::{GuestRestApi, StreamEvent, StreamHandle, TypedResponse};
