//! Chapter 4: comparing sequence numbers on a 32-bit circle.
//!
//! Owned by task `0004-final-handshake`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

/// `lhs` comes strictly before `rhs` on the sequence-number circle (RFC 1323).
///
/// Stub: see task 0004-final-handshake.
pub fn wrapping_lt(lhs: u32, rhs: u32) -> bool {
    todo!("task 0004-final-handshake")
}

/// `start < x < end` on the sequence-number circle, both bounds exclusive.
///
/// Stub: see task 0004-final-handshake.
pub fn is_between_wrapped(start: u32, x: u32, end: u32) -> bool {
    todo!("task 0004-final-handshake")
}
