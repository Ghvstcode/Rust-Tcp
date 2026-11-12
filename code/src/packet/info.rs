//! The TUN packet information prefix (chapter 2, first half).
//!
//! Owned by task `0002-parsing-the-bits`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use super::{PacketError, PacketInfo};

impl PacketInfo {
    /// Split the 4-byte prefix off a datagram read from a TUN device opened
    /// with packet information. Returns the prefix and the bytes after it.
    ///
    /// Stub: see task 0002-parsing-the-bits.
    pub fn parse(bytes: &[u8]) -> Result<(PacketInfo, &[u8]), PacketError> {
        todo!("task 0002-parsing-the-bits")
    }
}
