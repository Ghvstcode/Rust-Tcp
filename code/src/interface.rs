//! Chapter 5, the listening socket: `Interface`, `TcpListener`, `TcpStream`,
//! and the packet loop that runs behind them on its own thread.
//!
//! Owned by task `0005-listening-socket`. Stub: the public signatures are the
//! contract, the bodies (and every private item) are yours.
#![allow(unused_variables, dead_code, unused_imports)] // skeleton stub: delete this line when you implement the file

use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddrV4;
use std::sync::{Arc, Condvar, Mutex};

use crate::tcp::Connection;
use crate::{Nic, Quad};

/// Everything the packet loop and the application threads need to agree on.
#[derive(Default)]
struct ConnectionTable {
    connections: HashMap<Quad, Connection>,
    listeners: HashMap<u16, VecDeque<Quad>>,
    shutting_down: bool,
}

/// The table behind a lock, plus a way to wake whoever waits for a connection.
#[derive(Default)]
struct Shared {
    table: Mutex<ConnectionTable>,
    new_connection: Condvar,
}

/// Our handle to the network interface and the TCP stack running behind it.
pub struct Interface {
    shared: Arc<Shared>,
}

impl Interface {
    /// Open `tun0` without packet information and start the packet loop.
    pub fn new() -> io::Result<Interface> {
        todo!("task 0005-listening-socket")
    }

    /// Start the packet loop on any [`Nic`] (tests use an in-memory one).
    pub fn with_nic<N: Nic + Send + 'static>(nic: N) -> Interface {
        todo!("task 0005-listening-socket")
    }

    /// Start listening for connections on `port`.
    pub fn bind(&self, port: u16) -> io::Result<TcpListener> {
        todo!("task 0005-listening-socket")
    }
}

impl Drop for Interface {
    fn drop(&mut self) {
        todo!("task 0005-listening-socket")
    }
}

/// A socket bound to a port, waiting for connections.
pub struct TcpListener {
    port: u16,
    shared: Arc<Shared>,
}

impl TcpListener {
    /// Block until a connection on our port completes its handshake, then hand it over.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddrV4)> {
        todo!("task 0005-listening-socket")
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        todo!("task 0005-listening-socket")
    }
}

/// One end of an established TCP connection.
pub struct TcpStream {
    quad: Quad,
    shared: Arc<Shared>,
}

impl TcpStream {
    /// The address and port of the other end of the connection.
    pub fn peer_addr(&self) -> SocketAddrV4 {
        todo!("task 0005-listening-socket")
    }

    /// Our own address and port for this connection.
    pub fn local_addr(&self) -> SocketAddrV4 {
        todo!("task 0005-listening-socket")
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        // Chapter 6 sends a FIN here. Until then, dropping a stream does nothing.
    }
}
