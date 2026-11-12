//! Chapter 2, parsing the bits: the 4-byte TUN packet information, the IPv4
//! header and the TCP header.
//!
//! The book uses `etherparse` for the two headers. This companion implements
//! them itself, so the types are plain structs with public fields; the
//! accessor methods below carry the names the book's listings call
//! (`iph.source_addr()`, `tcph.destination_port()`, `tcph.syn()`, ...).
//!
//! This file (the types and accessors) is part of the skeleton and is in no
//! task's write scope. The parsing, writing and checksums live in `info.rs`,
//! `ipv4.rs` and `tcp_header.rs`, owned by task `0002-parsing-the-bits`.

use std::fmt;
use std::net::Ipv4Addr;

mod info;
mod ipv4;
mod tcp_header;

pub use ipv4::internet_checksum;

/// EtherType of IPv4, as found in [`PacketInfo::proto`].
pub const ETHERTYPE_IPV4: u16 = 0x0800;
/// EtherType of IPv6 (what a fresh `tun0` mostly sees: router solicitations).
pub const ETHERTYPE_IPV6: u16 = 0x86dd;
/// IP protocol number of TCP.
pub const PROTO_TCP: u8 = 6;

/// What can go wrong while reading or writing a header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PacketError {
    /// The slice is shorter than the header needs (`needed` bytes, `got` bytes).
    Truncated { needed: usize, got: usize },
    /// The IPv4 version field is not 4.
    NotIpv4 { version: u8 },
    /// IPv4 IHL or TCP data offset is below 5 (32-bit words).
    BadHeaderLength { value: u8 },
    /// IPv4 total length is smaller than the IPv4 header itself.
    BadTotalLength { total_len: u16, header_len: usize },
    /// A length that does not fit in its header field.
    TooLarge { len: usize, max: usize },
    /// Options that are longer than 40 bytes or not a multiple of 4 bytes.
    BadOptions { len: usize },
}

impl fmt::Display for PacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PacketError::Truncated { needed, got } => {
                write!(f, "truncated: need {} bytes, got {}", needed, got)
            }
            PacketError::NotIpv4 { version } => write!(f, "IP version {} is not 4", version),
            PacketError::BadHeaderLength { value } => {
                write!(f, "header length {} is below the minimum of 5 words", value)
            }
            PacketError::BadTotalLength { total_len, header_len } => write!(
                f,
                "total length {} is smaller than the {}-byte header",
                total_len, header_len
            ),
            PacketError::TooLarge { len, max } => write!(f, "length {} exceeds {}", len, max),
            PacketError::BadOptions { len } => write!(f, "{} bytes of options is not valid", len),
        }
    }
}

impl std::error::Error for PacketError {}

/// The 4 bytes a TUN device prepends to every datagram unless it was opened
/// with `IFF_NO_PI` (see the TUN/TAP documentation, section 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketInfo {
    /// Flags, e.g. `TUN_PKT_STRIP` when the kernel had to truncate the packet.
    pub flags: u16,
    /// The EtherType of what follows: [`ETHERTYPE_IPV4`], [`ETHERTYPE_IPV6`], ...
    pub proto: u16,
}

/// An IPv4 header (RFC 791, section 3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ipv4Header {
    /// The old type-of-service byte (DSCP and ECN), copied as is.
    pub dscp_ecn: u8,
    /// Length of the whole datagram: header, options and payload.
    pub total_len: u16,
    pub identification: u16,
    pub dont_fragment: bool,
    pub more_fragments: bool,
    /// Fragment offset in units of 8 bytes (13 bits).
    pub fragment_offset: u16,
    pub ttl: u8,
    /// The protocol of the payload; [`PROTO_TCP`] for TCP.
    pub protocol: u8,
    /// The checksum as read by `parse`. `write` ignores it and writes a freshly computed one.
    pub header_checksum: u16,
    pub source: [u8; 4],
    pub destination: [u8; 4],
    /// Raw option bytes: 0 to 40 bytes, a multiple of 4.
    pub options: Vec<u8>,
}

impl Ipv4Header {
    pub fn source_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.source)
    }

    pub fn destination_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.destination)
    }

    pub fn protocol(&self) -> u8 {
        self.protocol
    }
}

/// A TCP header (RFC 793, section 3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcpHeader {
    pub source_port: u16,
    pub destination_port: u16,
    pub sequence_number: u32,
    pub acknowledgment_number: u32,
    pub ns: bool,
    pub cwr: bool,
    pub ece: bool,
    pub urg: bool,
    pub ack: bool,
    pub psh: bool,
    pub rst: bool,
    pub syn: bool,
    pub fin: bool,
    pub window_size: u16,
    /// Written exactly as stored: compute it with `calc_checksum_ipv4` first.
    pub checksum: u16,
    pub urgent_pointer: u16,
    /// Raw option bytes: 0 to 40 bytes, a multiple of 4.
    pub options: Vec<u8>,
}

impl TcpHeader {
    pub fn source_port(&self) -> u16 {
        self.source_port
    }

    pub fn destination_port(&self) -> u16 {
        self.destination_port
    }

    pub fn sequence_number(&self) -> u32 {
        self.sequence_number
    }

    pub fn acknowledgment_number(&self) -> u32 {
        self.acknowledgment_number
    }

    pub fn window_size(&self) -> u16 {
        self.window_size
    }

    pub fn syn(&self) -> bool {
        self.syn
    }

    pub fn ack(&self) -> bool {
        self.ack
    }

    pub fn fin(&self) -> bool {
        self.fin
    }

    pub fn rst(&self) -> bool {
        self.rst
    }
}
