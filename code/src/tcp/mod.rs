//! Chapters 3 and 4: the transmission control block.
//!
//! This is the book's `tcp.rs`: the `State` enum, the two sequence spaces and
//! the `Connection` that ties them together. The book keeps all of it private
//! to the library; here the fields are `pub` so each chapter can be tested on
//! its own. Applications should not need them.
//!
//! This file is part of the skeleton and is in no task's write scope. The
//! behaviour lives in one file per task:
//!
//! | File          | Task                    | What                                         |
//! | ------------- | ----------------------- | -------------------------------------------- |
//! | `accept.rs`   | `0003-connection-quad`  | `Connection::accept`: SYN in, SYN-ACK out    |
//! | `seq.rs`      | `0004-final-handshake`  | `wrapping_lt`, `is_between_wrapped`          |
//! | `write.rs`    | `0004-final-handshake`  | `Connection::write`: build and send a segment |
//! | `on_packet.rs`| `0004-final-handshake`  | `on_packet`, `close`, `send_rst`, `is_established` |

use std::collections::VecDeque;

use crate::packet::{Ipv4Header, TcpHeader};

mod accept;
mod on_packet;
mod seq;
mod write;

pub use seq::{is_between_wrapped, wrapping_lt};

/// The states of RFC 793's connection state diagram that this stack uses so
/// far. CLOSE-WAIT, CLOSING and LAST-ACK arrive with the chapters that need
/// them. The default is `Listen`, as in the book.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    /// No connection exists.
    Closed,
    /// Waiting for a connection attempt from a remote endpoint.
    #[default]
    Listen,
    /// We received a SYN and sent a SYN-ACK; waiting for the ACK.
    SynRcvd,
    /// The handshake is complete; both ends can send and receive.
    Estab,
    /// We sent a FIN; waiting for it to be acknowledged.
    FinWait1,
    /// Our FIN was acknowledged; waiting for the peer's FIN.
    FinWait2,
    /// Both FINs are acknowledged; waiting out 2MSL (forever, for now).
    TimeWait,
}

/// Send Sequence Space (RFC 793, section 3.2, figure 4).
///
/// ```text
///            1         2          3          4
///       ----------|----------|----------|----------
///              SND.UNA    SND.NXT    SND.UNA
///                                   +SND.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers of unacknowledged data
/// 3 - sequence numbers allowed for new data transmission
/// 4 - future sequence numbers which are not yet allowed
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SendSequenceSpace {
    /// SND.UNA: oldest sequence number not yet acknowledged.
    pub una: u32,
    /// SND.NXT: next sequence number to send.
    pub nxt: u32,
    /// SND.WND: send window.
    pub wnd: u16,
    /// SND.UP: urgent pointer in effect.
    pub up: bool,
    /// SND.WL1: segment sequence number used for the last window update.
    pub wl1: usize,
    /// SND.WL2: segment acknowledgment number used for the last window update.
    pub wl2: usize,
    /// ISS: initial send sequence number.
    pub iss: u32,
}

/// Receive Sequence Space (RFC 793, section 3.2, figure 5).
///
/// ```text
///                1          2          3
///            ----------|----------|----------
///                   RCV.NXT    RCV.NXT
///                             +RCV.WND
///
/// 1 - old sequence numbers which have been acknowledged
/// 2 - sequence numbers allowed for new reception
/// 3 - future sequence numbers which are not yet allowed
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecvSequenceSpace {
    /// RCV.NXT: next sequence number expected.
    pub nxt: u32,
    /// RCV.WND: receive window.
    pub wnd: u16,
    /// RCV.UP: urgent pointer in effect.
    pub up: bool,
    /// IRS: initial receive sequence number.
    pub irs: u32,
}

/// One TCP connection: its state, its sequence spaces, and the headers of the
/// segments it sends (kept around and updated, as in the book).
#[derive(Clone, Debug)]
pub struct Connection {
    pub state: State,
    pub send: SendSequenceSpace,
    pub recv: RecvSequenceSpace,
    /// The IPv4 header of every segment we send on this connection.
    pub ip: Ipv4Header,
    /// The TCP header of every segment we send; `write` fills in the numbers.
    pub tcp: TcpHeader,
    /// Bytes handed to us to send, from SND.UNA onwards (chapter 4; used in chapter 6).
    pub unacked: VecDeque<u8>,
    /// The sequence number of our FIN, once we have closed our side.
    pub closed_at: Option<u32>,
}
