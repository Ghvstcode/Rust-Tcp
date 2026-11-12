//! Chapter 3, the connection quad: answering a SYN with a SYN-ACK.
//!
//! Owned by task `0003-connection-quad`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables)] // skeleton stub: delete this line when you implement the file

use std::io;

use super::Connection;
use crate::packet::{Ipv4Header, TcpHeader};
use crate::Nic;

impl Connection {
    /// Handle the first segment of a connection we have no TCB for. A SYN
    /// starts a handshake: send a SYN-ACK and return the new connection in
    /// `SynRcvd`. Anything else is ignored (`Ok(None)`).
    ///
    /// Stub: see task 0003-connection-quad.
    pub fn accept(
        nic: &mut dyn Nic,
        iph: &Ipv4Header,
        tcph: &TcpHeader,
        data: &[u8],
    ) -> io::Result<Option<Connection>> {
        todo!("task 0003-connection-quad")
    }
}
