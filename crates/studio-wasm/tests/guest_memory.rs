#![allow(missing_docs)]

use studio_wasm::{
    AbiErrorCode, AbiLimits, EmitBridge, copy_bytes_from_guest, copy_bytes_to_guest,
    copy_utf8_from_guest,
};

#[test]
fn rejects_negative_overflowing_and_out_of_bounds_pointer_length_pairs() {
    let memory = [0_u8; 16];
    for (pointer, length) in [(-1, 1), (0, -1), (i32::MAX, i32::MAX), (15, 2), (17, 0)] {
        assert_eq!(
            copy_bytes_from_guest(&memory, pointer, length, 64)
                .unwrap_err()
                .code(),
            AbiErrorCode::PointerInvalid
        );
    }
}

#[test]
fn copies_in_and_out_without_retaining_guest_slices() {
    let mut memory = b"guest-memory".to_vec();
    let copied = copy_bytes_from_guest(&memory, 0, 5, 32).unwrap();
    memory[..5].fill(b'x');
    assert_eq!(copied, b"guest");

    copy_bytes_to_guest(&mut memory, 6, b"HOST", 32).unwrap();
    assert_eq!(&memory[6..10], b"HOST");
    assert_eq!(
        copy_bytes_to_guest(&mut memory, 10, b"toolong", 32)
            .unwrap_err()
            .code(),
        AbiErrorCode::PointerInvalid
    );
}

#[test]
fn validates_utf8_after_copying_and_enforces_message_limits_first() {
    let memory = [0x66, 0x6f, 0x80, 0x6f];
    assert_eq!(
        copy_utf8_from_guest(&memory, 0, 4, 4).unwrap_err().code(),
        AbiErrorCode::Utf8Invalid
    );
    assert_eq!(
        copy_utf8_from_guest(&memory, 0, 4, 3).unwrap_err().code(),
        AbiErrorCode::MessageTooLarge
    );
}

#[test]
fn emit_copies_and_queues_until_the_guest_call_returns() {
    let mut memory = br#"{"type":"log"}"#.to_vec();
    let mut bridge = EmitBridge::new(AbiLimits::default());

    bridge.begin_guest_call().unwrap();
    bridge
        .emit(&memory, 0, i32::try_from(memory.len()).unwrap())
        .unwrap();
    memory.fill(b'x');
    assert!(bridge.pop_ready().is_none());
    assert_eq!(
        bridge.begin_guest_call().unwrap_err().code(),
        AbiErrorCode::ReentrantCall
    );
    bridge.end_guest_call().unwrap();

    assert_eq!(bridge.pop_ready().unwrap(), br#"{"type":"log"}"#);
    assert!(bridge.pop_ready().is_none());
}

#[test]
fn emit_enforces_message_and_queue_budgets_without_partial_enqueue() {
    let limits = AbiLimits {
        max_message_bytes: 4,
        max_queued_messages: 1,
        max_queued_bytes: 4,
    };
    let mut bridge = EmitBridge::new(limits);
    bridge.begin_guest_call().unwrap();

    assert_eq!(
        bridge.emit(b"12345", 0, 5).unwrap_err().code(),
        AbiErrorCode::MessageTooLarge
    );
    bridge.emit(b"1234", 0, 4).unwrap();
    assert_eq!(
        bridge.emit(b"x", 0, 1).unwrap_err().code(),
        AbiErrorCode::QueueFull
    );
    bridge.end_guest_call().unwrap();
    assert_eq!(bridge.pop_ready().unwrap(), b"1234");
}
