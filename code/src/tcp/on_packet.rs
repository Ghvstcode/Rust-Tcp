//! Chapter 4, the final handshake: segment acceptance, the ACK that
//! establishes a connection, and closing it again.
//!
//! Owned by task `0004-final-handshake`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use std::io;

use super::Connection;
use crate::packet::{Ipv4Header, TcpHeader};
use crate::Nic;

impl Connection {
    /// Process one incoming segment for this connection.
    ///
    /// Stub: see task 0004-final-handshake.
    pub fn on_packet(
        &mut self,
        nic: &mut dyn Nic,
        iph: &Ipv4Header,
        tcph: &TcpHeader,
        data: &[u8],
    ) -> io::Result<()> {
        todo!("task 0004-final-handshake")
    }

    /// Has this connection completed the three-way handshake? (Chapter 5 uses it.)
    pub fn is_established(&self) -> bool {
        todo!("task 0004-final-handshake")
    }

    /// Close our side: send a FIN and move to `FinWait1`. This is the block
    /// chapter 5 removes from `on_packet`, given a name of its own.
    pub fn close(&mut self, nic: &mut dyn Nic) -> io::Result<()> {
        todo!("task 0004-final-handshake")
    }

    /// Send a reset segment.
    pub fn send_rst(&mut self, nic: &mut dyn Nic) -> io::Result<()> {
        todo!("task 0004-final-handshake")
    }
}
