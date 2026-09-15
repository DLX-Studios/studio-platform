#![no_main]
use libfuzzer_sys::fuzz_target;
use studio_protocol::{GuestMessage, ProtocolLimits, decode_guest_message};

fuzz_target!(|data: &[u8]| {
    if let Ok(GuestMessage::Action(action)) = decode_guest_message(data, ProtocolLimits::default()) {
        let _ = serde_json::to_vec(&action);
    }
});
