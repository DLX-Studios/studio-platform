//! Checked copy-in/copy-out helpers for untrusted wasm32 linear memory.

use crate::AbiError;

/// Copy a bounded byte range out of guest memory without retaining a guest slice.
///
/// # Errors
///
/// Returns [`AbiError`] for negative, overflowing, out-of-bounds, or oversized ranges.
pub fn copy_bytes_from_guest(
    memory: &[u8],
    pointer: i32,
    length: i32,
    maximum_bytes: usize,
) -> Result<Vec<u8>, AbiError> {
    let range = checked_range(memory.len(), pointer, length)?;
    if range.len() > maximum_bytes {
        return Err(AbiError::MessageTooLarge {
            actual: range.len(),
            limit: maximum_bytes,
        });
    }
    Ok(memory[range].to_vec())
}

/// Copy bounded host bytes into guest memory without retaining a host or guest slice.
///
/// # Errors
///
/// Returns [`AbiError`] for negative, overflowing, out-of-bounds, or oversized ranges.
pub fn copy_bytes_to_guest(
    memory: &mut [u8],
    pointer: i32,
    source: &[u8],
    maximum_bytes: usize,
) -> Result<(), AbiError> {
    let length = i32::try_from(source.len()).map_err(|_| AbiError::PointerInvalid {
        pointer,
        length: i32::MAX,
        memory_bytes: memory.len(),
    })?;
    let range = checked_range(memory.len(), pointer, length)?;
    if source.len() > maximum_bytes {
        return Err(AbiError::MessageTooLarge {
            actual: source.len(),
            limit: maximum_bytes,
        });
    }
    memory[range].copy_from_slice(source);
    Ok(())
}

/// Copy and validate a bounded UTF-8 string from guest memory.
///
/// # Errors
///
/// Returns [`AbiError`] for invalid ranges, budget excess, or invalid UTF-8.
pub fn copy_utf8_from_guest(
    memory: &[u8],
    pointer: i32,
    length: i32,
    maximum_bytes: usize,
) -> Result<String, AbiError> {
    let bytes = copy_bytes_from_guest(memory, pointer, length, maximum_bytes)?;
    String::from_utf8(bytes).map_err(|_| AbiError::Utf8Invalid)
}

fn checked_range(
    memory_bytes: usize,
    pointer: i32,
    length: i32,
) -> Result<std::ops::Range<usize>, AbiError> {
    let pointer_u32 = u32::try_from(pointer).map_err(|_| AbiError::PointerInvalid {
        pointer,
        length,
        memory_bytes,
    })?;
    let length_u32 = u32::try_from(length).map_err(|_| AbiError::PointerInvalid {
        pointer,
        length,
        memory_bytes,
    })?;
    let end_u32 = pointer_u32
        .checked_add(length_u32)
        .ok_or(AbiError::PointerInvalid {
            pointer,
            length,
            memory_bytes,
        })?;
    let start = usize::try_from(pointer_u32).map_err(|_| AbiError::PointerInvalid {
        pointer,
        length,
        memory_bytes,
    })?;
    let end = usize::try_from(end_u32).map_err(|_| AbiError::PointerInvalid {
        pointer,
        length,
        memory_bytes,
    })?;
    if start > memory_bytes || end > memory_bytes {
        return Err(AbiError::PointerInvalid {
            pointer,
            length,
            memory_bytes,
        });
    }
    Ok(start..end)
}
