//! The IPv4 header and the Internet checksum (chapter 2).
//!
//! Owned by task `0002-parsing-the-bits`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use std::io;

use super::{Ipv4Header, PacketError};

/// The 16-bit one's complement of the one's complement sum of `data`
/// (RFC 1071). An odd trailing byte is padded with a zero byte.
///
/// Stub: see task 0002-parsing-the-bits.
pub fn internet_checksum(data: &[u8]) -> u16 {
    todo!("task 0002-parsing-the-bits")
}

impl Ipv4Header {
    /// A header for a new datagram, like `etherparse::Ipv4Header::new`.
    ///
    /// Stub: see task 0002-parsing-the-bits.
    pub fn new(payload_len: u16, ttl: u8, protocol: u8, source: [u8; 4], destination: [u8; 4]) -> Ipv4Header {
        todo!("task 0002-parsing-the-bits")
    }

    /// Read the header at the start of `bytes`.
    ///
    /// Stub: see task 0002-parsing-the-bits.
    pub fn parse(bytes: &[u8]) -> Result<Ipv4Header, PacketError> {
        todo!("task 0002-parsing-the-bits")
    }

    /// Header length in bytes, options included.
    pub fn header_len(&self) -> usize {
        todo!("task 0002-parsing-the-bits")
    }

    /// Payload length implied by `total_len`.
    pub fn payload_len(&self) -> usize {
        todo!("task 0002-parsing-the-bits")
    }

    /// Set `total_len` so the datagram carries `len` bytes of payload.
    pub fn set_payload_len(&mut self, len: usize) -> Result<(), PacketError> {
        todo!("task 0002-parsing-the-bits")
    }

    /// The header checksum this header should carry.
    pub fn calc_header_checksum(&self) -> u16 {
        todo!("task 0002-parsing-the-bits")
    }

    /// Write the header, with a freshly computed checksum, to `w`.
    pub fn write<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        todo!("task 0002-parsing-the-bits")
    }
}
