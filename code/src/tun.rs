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
//! Owned by task `0001-tun-device`.

use std::fs::File;
use std::io::{self, Read, Write};

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

/// Build the `struct ifreq` passed to `TUNSETIFF`: the name, NUL-padded, then
/// `ifr_flags` in native byte order, then zeros.
///
/// The name may be at most 15 bytes and must not contain a NUL byte.
pub fn ifreq(name: &str, mode: Mode, packet_info: bool) -> io::Result<[u8; IFREQ_LEN]> {
    let bytes = name.as_bytes();
    if bytes.len() >= IFNAMSIZ || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "interface name must be at most 15 bytes and contain no NUL",
        ));
    }
    let mut flags = match mode {
        Mode::Tun => IFF_TUN,
        Mode::Tap => IFF_TAP,
    };
    if !packet_info {
        flags |= IFF_NO_PI;
    }
    let mut ifr = [0u8; IFREQ_LEN];
    ifr[..bytes.len()].copy_from_slice(bytes);
    ifr[IFNAMSIZ..IFNAMSIZ + 2].copy_from_slice(&flags.to_ne_bytes());
    Ok(ifr)
}

/// Read the interface name back out of an `ifreq` (the kernel fills it in).
pub fn ifreq_name(ifr: &[u8; IFREQ_LEN]) -> io::Result<String> {
    let field = &ifr[..IFNAMSIZ];
    let end = field.iter().position(|&b| b == 0).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "interface name is not NUL-terminated")
    })?;
    String::from_utf8(field[..end].to_vec())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// An open TUN (or TAP) interface, the equivalent of `tun_tap::Iface`.
pub struct Iface {
    file: File,
    name: String,
}

#[cfg(target_os = "linux")]
extern "C" {
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

impl Iface {
    /// Open `name` with packet information: every datagram read starts with
    /// the 4 bytes of flags and protocol that chapter 2 parses.
    pub fn new(name: &str, mode: Mode) -> io::Result<Iface> {
        Iface::open(name, mode, true)
    }

    /// Open `name` without packet information (`IFF_NO_PI`): what the book
    /// uses from chapter 3 on.
    pub fn without_packet_info(name: &str, mode: Mode) -> io::Result<Iface> {
        Iface::open(name, mode, false)
    }

    #[cfg(target_os = "linux")]
    fn open(name: &str, mode: Mode, packet_info: bool) -> io::Result<Iface> {
        use std::os::unix::io::AsRawFd;

        let mut ifr = ifreq(name, mode, packet_info)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/net/tun")?;
        // SAFETY: the fd is open for the duration of the call and `ifr` is a
        // live, writable 40-byte buffer, the size of `struct ifreq`.
        let ret = unsafe { ioctl(file.as_raw_fd(), TUNSETIFF, ifr.as_mut_ptr()) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        let name = ifreq_name(&ifr)?;
        Ok(Iface { file, name })
    }

    #[cfg(not(target_os = "linux"))]
    fn open(name: &str, mode: Mode, packet_info: bool) -> io::Result<Iface> {
        ifreq(name, mode, packet_info)?;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "TUN devices are only supported on Linux",
        ))
    }

    /// The interface name the kernel actually used (it may differ from the
    /// requested one, e.g. `"tun%d"` becomes `"tun0"`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Read one datagram into `buf`; returns its length.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.file).read(buf)
    }

    /// Write one datagram; returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        (&self.file).write(buf)
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
