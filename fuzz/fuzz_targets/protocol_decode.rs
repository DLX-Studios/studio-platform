#![no_main]
use libfuzzer_sys::fuzz_target;
use studio_protocol::{ProtocolLimits, decode_guest_message, decode_host_event};

fuzz_target!(|data: &[u8]| {
    let _ = decode_guest_message(data, ProtocolLimits::default());
    let _ = decode_host_event(data, ProtocolLimits::default());
});
