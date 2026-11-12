//! Chapter 4: the `write` helper that builds and sends every segment.
//!
//! Owned by task `0004-final-handshake`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use std::io;

use super::Connection;
use crate::Nic;

impl Connection {
    /// Send one segment starting at sequence number `seq`, carrying at most
    /// `limit` bytes from `unacked`. Returns the number of payload bytes sent.
    ///
    /// Stub: see task 0004-final-handshake.
    pub fn write(&mut self, nic: &mut dyn Nic, seq: u32, limit: usize) -> io::Result<usize> {
        todo!("task 0004-final-handshake")
    }
}
