//! A user-space TCP stack, built chapter by chapter alongside the book
//! *Implementing TCP in Rust* (<https://github.com/Ghvstcode/Rust-Tcp>).
//!
//! This crate is the code companion: it ends up where the book's chapter 5
//! leaves off. Each module says which chapter it belongs to.
//!
//! | Module          | Chapter | What it is                                              |
//! | --------------- | ------- | ------------------------------------------------------- |
//! | [`tun`]         | 1       | The TUN device (what the book gets from `tun-tap`)      |
//! | [`packet`]      | 2       | IPv4 and TCP headers (what the book gets from `etherparse`) |
//! | [`tcp`]         | 3, 4    | The TCB: `Connection`, `State`, the sequence spaces     |
//! | [`Interface`]   | 5       | `Interface`, `TcpListener`, `TcpStream`                 |
//!
//! The book keeps `tcp::Connection` private to the library. Here it is `pub`,
//! so that each chapter can be tested on its own, byte by byte, without a TUN
//! device. Applications should still only need [`Interface`], [`TcpListener`]
//! and [`TcpStream`].
//!
//! This file is part of the skeleton and is in no task's write scope.

use std::io;
use std::net::Ipv4Addr;

pub mod packet;
pub mod tcp;
pub mod tun;

mod interface;

pub use interface::{Interface, TcpListener, TcpStream};

/// Anything that moves raw IPv4 datagrams in and out: the TUN device in
/// production ([`tun::Iface`]), an in-memory fake in tests.
///
/// The two methods mirror `tun_tap::Iface::recv` and `tun_tap::Iface::send`,
/// so every listing in the book translates one for one: where the book passes
/// `nic: &mut tun_tap::Iface`, this crate passes `nic: &mut dyn Nic`.
pub trait Nic {
    /// Block until one datagram arrives, copy it into `buf`, return its length.
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    /// Send one complete datagram (IPv4 header, TCP header, payload).
    fn send(&mut self, buf: &[u8]) -> io::Result<usize>;
}

/// The connection quad from chapter 3: it uniquely identifies a TCP connection
/// and is the key of the connection table.
///
/// `src` is the remote end (who sent us the packet), `dst` is our end (the
/// address and port they were trying to reach).
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct Quad {
    pub src: (Ipv4Addr, u16),
    pub dst: (Ipv4Addr, u16),
}
