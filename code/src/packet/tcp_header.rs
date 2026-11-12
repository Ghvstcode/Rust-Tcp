//! The TCP header and its checksum (chapter 2).
//!
//! Owned by task `0002-parsing-the-bits`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use std::io;

use super::{Ipv4Header, PacketError, TcpHeader};

impl TcpHeader {
    /// A header with no flags set, like `etherparse::TcpHeader::new`.
    ///
    /// Stub: see task 0002-parsing-the-bits.
    pub fn new(source_port: u16, destination_port: u16, sequence_number: u32, window_size: u16) -> TcpHeader {
        todo!("task 0002-parsing-the-bits")
    }

    /// Read the header at the start of `bytes`.
    pub fn parse(bytes: &[u8]) -> Result<TcpHeader, PacketError> {
        todo!("task 0002-parsing-the-bits")
    }

    /// Header length in bytes, options included.
    pub fn header_len(&self) -> usize {
        todo!("task 0002-parsing-the-bits")
    }

    /// The checksum this header should carry, over the IPv4 pseudo-header,
    /// this header and `payload`.
    pub fn calc_checksum_ipv4(&self, ip: &Ipv4Header, payload: &[u8]) -> Result<u16, PacketError> {
        todo!("task 0002-parsing-the-bits")
    }

    /// Write the header to `w`, with `self.checksum` exactly as stored.
    pub fn write<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
        todo!("task 0002-parsing-the-bits")
    }
}
