//! Protobuf and C bindings for the compiled fictional bonus policy.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

use prost::Message;

mod bonus;

// Reuse the executable policy, including its fixture helpers for conformance tests.
#[allow(dead_code)]
#[path = "../../examples/support/bonus.rs"]
mod policy;

/// Generated transport types. Fact presence is represented by `Option`.
#[allow(missing_docs)]
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/inferrum.bonus.v1.rs"));
}

/// Opaque, immutable engine handle. Create and release it through this library.
pub struct InferrumEngine {
    policy: bonus::BonusPolicy,
}

/// Rust-owned bytes. Release exactly once with [`inferrum_buffer_free`].
#[repr(C)]
pub struct InferrumBuffer {
    /// Pointer to `len` bytes, or null for an empty buffer.
    pub data: *mut u8,
    /// Byte length. Do not alter it before releasing the buffer.
    pub len: usize,
}

impl InferrumBuffer {
    fn empty() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
        }
    }

    fn encode(message: impl Message) -> Self {
        let bytes = message.encode_to_vec().into_boxed_slice();
        if bytes.is_empty() {
            return Self::empty();
        }
        let len = bytes.len();
        Self {
            data: Box::into_raw(bytes).cast::<u8>(),
            len,
        }
    }
}

/// An owned engine on success, or an encoded `EvaluationError` on failure.
#[repr(C)]
pub struct InferrumCreateResult {
    /// Null on failure. Release a live handle with [`inferrum_engine_free`].
    pub engine: *mut InferrumEngine,
    /// Empty on success. Release error bytes with [`inferrum_buffer_free`].
    pub error: InferrumBuffer,
}

// An arbitrary panic payload can itself panic on drop. Keep it from unwinding
// across C; normal responses and errors do not use this path.
fn contain_panic<T>(operation: impl FnOnce() -> T) -> Result<T, wire::EvaluationError> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|payload| {
        std::mem::forget(payload);
        bonus::error(wire::ErrorCode::Internal, "Rust evaluation panicked")
    })
}

/// Validates the compiled bonus policy once and creates an owned handle.
#[unsafe(no_mangle)]
pub extern "C" fn inferrum_bonus_new() -> InferrumCreateResult {
    match contain_panic(|| {
        bonus::BonusPolicy::new().map(|policy| Box::into_raw(Box::new(InferrumEngine { policy })))
    })
    .and_then(|result| result)
    {
        Ok(engine) => InferrumCreateResult {
            engine,
            error: InferrumBuffer::empty(),
        },
        Err(error) => InferrumCreateResult {
            engine: ptr::null_mut(),
            error: InferrumBuffer::encode(error),
        },
    }
}

/// Evaluates a complete request and returns an owned encoded `BonusResponse`.
/// Null pointers and invalid lengths produce structured errors.
///
/// # Safety
/// A non-null engine must be a live handle from [`inferrum_bonus_new`]. It must
/// not be freed during this call. A non-null request must point to `request_len`
/// readable bytes that remain unchanged until this call returns. Request bytes
/// are never retained. Concurrent evaluations may share a handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn inferrum_bonus_evaluate(
    engine: *const InferrumEngine,
    request: *const u8,
    request_len: usize,
) -> InferrumBuffer {
    let result = contain_panic(|| {
        if engine.is_null()
            || (request.is_null() && request_len != 0)
            || request_len > isize::MAX as usize
        {
            return bonus::response(Err(bonus::error(
                wire::ErrorCode::InvalidInput,
                "invalid engine, request pointer, or request length",
            )));
        }
        let bytes = if request_len == 0 {
            &[]
        } else {
            // SAFETY: The caller supplies readable bytes; null and oversized lengths were rejected.
            unsafe { std::slice::from_raw_parts(request, request_len) }
        };
        match wire::BonusRequest::decode(bytes) {
            Ok(request) => {
                // SAFETY: The caller keeps this handle alive for the entire evaluation.
                unsafe { &*engine }.policy.evaluate(request)
            }
            Err(source) => bonus::response(Err(bonus::error(
                wire::ErrorCode::Decode,
                source.to_string(),
            ))),
        }
    });
    InferrumBuffer::encode(result.unwrap_or_else(|error| bonus::response(Err(error))))
}

/// Releases response or construction-error bytes. An empty null buffer is allowed.
///
/// # Safety
/// A non-null buffer must be an unchanged, still-owned buffer returned by this
/// library. Release it exactly once and do not access its bytes afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn inferrum_buffer_free(buffer: InferrumBuffer) {
    if !buffer.data.is_null() {
        // SAFETY: The caller returns the exact pointer and length from Box<[u8]>.
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(buffer.data, buffer.len)) });
    }
}

/// Releases an engine handle. Null is allowed.
///
/// # Safety
/// A non-null handle must come from [`inferrum_bonus_new`] and be released exactly
/// once. All evaluations must have finished before release begins.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn inferrum_engine_free(engine: *mut InferrumEngine) {
    if !engine.is_null() {
        let _ = contain_panic(|| {
            // SAFETY: The caller transfers exclusive ownership of the live handle back to Rust.
            drop(unsafe { Box::from_raw(engine) });
        });
    }
}

#[cfg(test)]
mod tests;
