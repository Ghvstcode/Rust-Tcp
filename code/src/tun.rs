//! Chapter 1, the kernel-user space divide: a TUN device, opened with nothing
//! but the standard library.
//!
//! The book uses the `tun-tap` crate. Underneath, that crate opens
//! `/dev/net/tun` and issues one `ioctl(TUNSETIFF)` with a `struct ifreq`
//! naming the interface and its flags. This module does the same. The pure
//! part (building and reading the `ifreq` bytes, validating the name) is what
//! the acceptance test checks; opening the device needs Linux and
//! `cap_net_admin`, so it is compiled but never run in CI.
//!
//! Owned by task `0001-tun-device`. Stub: the signatures are the contract, the bodies are yours.
#![allow(unused_variables, dead_code)] // skeleton stub: delete this line when you implement the file

use std::fs::File;
use std::io;

use crate::Nic;

/// `ioctl` request number that attaches a file descriptor to a TUN/TAP interface (Linux).
pub const TUNSETIFF: u64 = 0x4004_54ca;
/// `ifr_flags` bit: a TUN device (layer 3, IP datagrams).
pub const IFF_TUN: i16 = 0x0001;
/// `ifr_flags` bit: a TAP device (layer 2, Ethernet frames).
pub const IFF_TAP: i16 = 0x0002;
/// `ifr_flags` bit: do not prepend the 4-byte packet information header (chapter 2).
pub const IFF_NO_PI: i16 = 0x1000;
/// Size of the interface-name field of `struct ifreq`, including the terminating NUL.
pub const IFNAMSIZ: usize = 16;
/// Size of `struct ifreq` on 64-bit Linux.
pub const IFREQ_LEN: usize = 40;

/// Which kind of virtual interface to create. The book only ever uses `Tun`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Tun,
    Tap,
}

/// Build the `struct ifreq` passed to `TUNSETIFF`.
///
/// Stub: see task 0001-tun-device for the exact layout and error rules.
pub fn ifreq(name: &str, mode: Mode, packet_info: bool) -> io::Result<[u8; IFREQ_LEN]> {
    todo!("task 0001-tun-device")
}

/// Read the interface name back out of an `ifreq` (the kernel fills it in).
///
/// Stub: see task 0001-tun-device.
pub fn ifreq_name(ifr: &[u8; IFREQ_LEN]) -> io::Result<String> {
    todo!("task 0001-tun-device")
}

/// An open TUN (or TAP) interface, the equivalent of `tun_tap::Iface`.
pub struct Iface {
    file: File,
    name: String,
}

impl Iface {
    /// Open `name` with packet information: every datagram read starts with
    /// the 4 bytes of flags and protocol that chapter 2 parses.
    pub fn new(name: &str, mode: Mode) -> io::Result<Iface> {
        todo!("task 0001-tun-device")
    }

    /// Open `name` without packet information (`IFF_NO_PI`): what the book
    /// uses from chapter 3 on.
    pub fn without_packet_info(name: &str, mode: Mode) -> io::Result<Iface> {
        todo!("task 0001-tun-device")
    }

    /// The interface name the kernel actually used (it may differ from the
    /// requested one, e.g. `"tun%d"` becomes `"tun0"`).
    pub fn name(&self) -> &str {
        todo!("task 0001-tun-device")
    }

    /// Read one datagram into `buf`; returns its length.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        todo!("task 0001-tun-device")
    }

    /// Write one datagram; returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        todo!("task 0001-tun-device")
    }
}

impl Nic for Iface {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Iface::recv(self, buf)
    }

    fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        Iface::send(self, buf)
    }
}
