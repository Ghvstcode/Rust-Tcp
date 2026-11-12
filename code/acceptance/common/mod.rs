//! Shared by the acceptance tests. Outside every task's write scope.
//!
//! The judge brings its own packet codec, written independently of
//! `tcp::packet`, so a test of chapters 3 to 5 checks the bytes on the wire
//! rather than trusting the chapter 2 implementation to read back what it
//! wrote. It also brings the fake network interfaces the tests use instead of
//! a TUN device.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tcp::packet::{Ipv4Header, TcpHeader};
use tcp::tcp::{Connection, RecvSequenceSpace, SendSequenceSpace, State};
use tcp::Nic;

pub const FIN: u8 = 0x01;
pub const SYN: u8 = 0x02;
pub const RST: u8 = 0x04;
pub const PSH: u8 = 0x08;
pub const ACK: u8 = 0x10;

/// The client end of every test connection (the kernel's side of tun0 in the book).
pub const CLIENT: [u8; 4] = [192, 168, 0, 1];
/// Our end (the address the book's stack answers for).
pub const SERVER: [u8; 4] = [192, 168, 0, 2];

/// RFC 1071 checksum, written out the slow and obvious way.
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u64 = 0;
    let mut i = 0;
    while i < data.len() {
        let hi = data[i] as u64;
        let lo = if i + 1 < data.len() { data[i + 1] as u64 } else { 0 };
        sum += (hi << 8) | lo;
        i += 2;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// A segment to put on the wire, or one taken off it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg {
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub sport: u16,
    pub dport: u16,
    pub seq: u32,
    pub ack: u32,
    pub flags: u8,
    pub window: u16,
    pub payload: Vec<u8>,
}

impl Seg {
    /// A segment from the client to the server.
    pub fn client(sport: u16, dport: u16, seq: u32, ack: u32, flags: u8) -> Seg {
        Seg {
            src: CLIENT,
            dst: SERVER,
            sport,
            dport,
            seq,
            ack,
            flags,
            window: 64240,
            payload: Vec::new(),
        }
    }

    pub fn with_payload(mut self, payload: &[u8]) -> Seg {
        self.payload = payload.to_vec();
        self
    }

    pub fn with_window(mut self, window: u16) -> Seg {
        self.window = window;
        self
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    /// Encode as a complete IPv4 datagram with correct checksums.
    pub fn encode(&self) -> Vec<u8> {
        let tcp_len = 20 + self.payload.len();
        let total = 20 + tcp_len;
        let mut ip = vec![
            0x45,
            0,
            (total >> 8) as u8,
            total as u8,
            0x12,
            0x34,
            0x40,
            0,
            64,
            6,
            0,
            0,
        ];
        ip.extend_from_slice(&self.src);
        ip.extend_from_slice(&self.dst);
        let c = checksum(&ip);
        ip[10] = (c >> 8) as u8;
        ip[11] = c as u8;

        let mut tcp = Vec::new();
        tcp.extend_from_slice(&self.sport.to_be_bytes());
        tcp.extend_from_slice(&self.dport.to_be_bytes());
        tcp.extend_from_slice(&self.seq.to_be_bytes());
        tcp.extend_from_slice(&self.ack.to_be_bytes());
        tcp.push(5 << 4);
        tcp.push(self.flags);
        tcp.extend_from_slice(&self.window.to_be_bytes());
        tcp.extend_from_slice(&[0, 0, 0, 0]);
        tcp.extend_from_slice(&self.payload);
        let c = tcp_checksum(&self.src, &self.dst, &tcp);
        tcp[16] = (c >> 8) as u8;
        tcp[17] = c as u8;

        ip.extend_from_slice(&tcp);
        ip
    }

    /// The same segment as the crate's header types, for calling
    /// `Connection::accept` / `on_packet` directly. Built field by field, so
    /// it does not depend on the chapter 2 parser.
    pub fn headers(&self) -> (Ipv4Header, TcpHeader) {
        let ip = Ipv4Header {
            dscp_ecn: 0,
            total_len: (40 + self.payload.len()) as u16,
            identification: 0x1234,
            dont_fragment: true,
            more_fragments: false,
            fragment_offset: 0,
            ttl: 64,
            protocol: 6,
            header_checksum: 0,
            source: self.src,
            destination: self.dst,
            options: Vec::new(),
        };
        let tcp = TcpHeader {
            source_port: self.sport,
            destination_port: self.dport,
            sequence_number: self.seq,
            acknowledgment_number: self.ack,
            ns: false,
            cwr: false,
            ece: false,
            urg: false,
            ack: self.has(ACK),
            psh: self.has(PSH),
            rst: self.has(RST),
            syn: self.has(SYN),
            fin: self.has(FIN),
            window_size: self.window,
            checksum: 0,
            urgent_pointer: 0,
            options: Vec::new(),
        };
        (ip, tcp)
    }
}

fn tcp_checksum(src: &[u8; 4], dst: &[u8; 4], tcp: &[u8]) -> u16 {
    let mut pseudo = Vec::new();
    pseudo.extend_from_slice(src);
    pseudo.extend_from_slice(dst);
    pseudo.push(0);
    pseudo.push(6);
    pseudo.extend_from_slice(&(tcp.len() as u16).to_be_bytes());
    pseudo.extend_from_slice(tcp);
    checksum(&pseudo)
}

/// Decode a datagram the stack sent, checking everything a real peer would:
/// version, lengths, protocol and both checksums. Panics with a readable
/// message when the datagram is not a well-formed IPv4/TCP segment.
pub fn decode(bytes: &[u8]) -> Seg {
    assert!(bytes.len() >= 40, "datagram of {} bytes is too short for IPv4 + TCP: {:02x?}", bytes.len(), bytes);
    assert_eq!(bytes[0] >> 4, 4, "IP version must be 4: {:02x?}", bytes);
    let ihl = (bytes[0] & 0x0f) as usize * 4;
    assert!(ihl >= 20 && bytes.len() >= ihl + 20, "bad IHL {} in {:02x?}", ihl, bytes);
    let total = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
    assert_eq!(total, bytes.len(), "IPv4 total length must equal the datagram length sent");
    assert_eq!(checksum(&bytes[..ihl]), 0, "IPv4 header checksum is wrong: {:02x?}", &bytes[..ihl]);
    assert_eq!(bytes[9], 6, "IP protocol must be 6 (TCP)");
    let src = [bytes[12], bytes[13], bytes[14], bytes[15]];
    let dst = [bytes[16], bytes[17], bytes[18], bytes[19]];
    let tcp = &bytes[ihl..];
    let off = (tcp[12] >> 4) as usize * 4;
    assert!(off >= 20 && tcp.len() >= off, "bad TCP data offset {} in {:02x?}", off, tcp);
    assert_eq!(tcp_checksum(&src, &dst, tcp), 0, "TCP checksum is wrong: {:02x?}", bytes);
    Seg {
        src,
        dst,
        sport: u16::from_be_bytes([tcp[0], tcp[1]]),
        dport: u16::from_be_bytes([tcp[2], tcp[3]]),
        seq: u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]),
        ack: u32::from_be_bytes([tcp[8], tcp[9], tcp[10], tcp[11]]),
        flags: tcp[13],
        window: u16::from_be_bytes([tcp[14], tcp[15]]),
        payload: tcp[off..].to_vec(),
    }
}

/// A NIC that records what is sent and never receives. For calling
/// `Connection` methods directly.
#[derive(Default)]
pub struct RecordingNic {
    pub sent: Vec<Vec<u8>>,
}

impl RecordingNic {
    pub fn new() -> RecordingNic {
        RecordingNic::default()
    }

    /// Decode and remove everything sent so far.
    pub fn take(&mut self) -> Vec<Seg> {
        self.sent.drain(..).map(|d| decode(&d)).collect()
    }

    /// Exactly one segment must have been sent; decode and return it.
    pub fn take_one(&mut self) -> Seg {
        let mut segs = self.take();
        assert_eq!(segs.len(), 1, "expected exactly one segment to be sent, got {}: {:?}", segs.len(), segs);
        segs.remove(0)
    }
}

impl Nic for RecordingNic {
    fn recv(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("RecordingNic never receives"))
    }

    fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sent.push(buf.to_vec());
        Ok(buf.len())
    }
}

/// A NIC backed by channels, for running the real packet loop on its own thread.
pub struct ChannelNic {
    incoming: Receiver<Vec<u8>>,
    outgoing: Sender<Vec<u8>>,
}

impl Nic for ChannelNic {
    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.incoming.recv() {
            Ok(d) => {
                let n = d.len().min(buf.len());
                buf[..n].copy_from_slice(&d[..n]);
                Ok(n)
            }
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "test finished")),
        }
    }

    fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        // The test may have finished and dropped its receiver; that is fine.
        let _ = self.outgoing.send(buf.to_vec());
        Ok(buf.len())
    }
}

/// The test's end of a `ChannelNic`: inject datagrams, watch what the stack sends.
pub struct Wire {
    to_stack: Sender<Vec<u8>>,
    from_stack: Receiver<Vec<u8>>,
    /// Segments received but not yet asked for.
    pending: VecDeque<Seg>,
}

pub fn channel_nic() -> (ChannelNic, Wire) {
    let (to_stack, incoming) = mpsc::channel();
    let (outgoing, from_stack) = mpsc::channel();
    (
        ChannelNic { incoming, outgoing },
        Wire { to_stack, from_stack, pending: VecDeque::new() },
    )
}

/// How long a test waits for something it expects. Generous, but a broken
/// implementation still fails in seconds instead of hanging.
pub const PATIENCE: Duration = Duration::from_secs(5);

impl Wire {
    pub fn inject(&self, datagram: Vec<u8>) {
        self.to_stack.send(datagram).expect("the packet loop has stopped reading");
    }

    pub fn send(&self, seg: &Seg) {
        self.inject(seg.encode());
    }

    /// Wait for the first segment the stack sends that satisfies `pred`,
    /// skipping (but keeping) any others. Panics after `PATIENCE`.
    pub fn expect(&mut self, what: &str, pred: impl Fn(&Seg) -> bool) -> Seg {
        if let Some(i) = self.pending.iter().position(&pred) {
            return self.pending.remove(i).unwrap();
        }
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.from_stack.recv_timeout(left) {
                Ok(d) => {
                    let seg = decode(&d);
                    if pred(&seg) {
                        return seg;
                    }
                    self.pending.push_back(seg);
                }
                Err(RecvTimeoutError::Timeout) => panic!(
                    "timed out after {:?} waiting for the stack to send {}; it sent only {:?}",
                    PATIENCE, what, self.pending
                ),
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("the packet loop stopped while we waited for {}", what)
                }
            }
        }
    }

    /// Wait for the SYN-ACK answering a SYN from `sport` to `dport`.
    pub fn expect_syn_ack(&mut self, sport: u16, dport: u16) -> Seg {
        self.expect(&format!("a SYN-ACK from port {} to port {}", dport, sport), |s| {
            s.sport == dport && s.dport == sport && s.has(SYN) && s.has(ACK)
        })
    }

    /// Run a full client-side handshake from `sport` to `dport` and return
    /// the server's initial sequence number.
    pub fn handshake(&mut self, sport: u16, dport: u16, client_isn: u32) -> u32 {
        self.send(&Seg::client(sport, dport, client_isn, 0, SYN));
        let syn_ack = self.expect_syn_ack(sport, dport);
        assert_eq!(syn_ack.ack, client_isn.wrapping_add(1), "the SYN-ACK must acknowledge our SYN");
        self.send(&Seg::client(sport, dport, client_isn.wrapping_add(1), syn_ack.seq.wrapping_add(1), ACK));
        syn_ack.seq
    }
}

/// Run `f` on another thread and wait at most `PATIENCE` for its result.
/// `None` means it did not return in time (it is left running).
pub fn within<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    within_for(PATIENCE, f)
}

pub fn within_for<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(limit).ok()
}

/// A connection built field by field in the given state, as `accept` would
/// leave it after a SYN from `CLIENT:sport` with sequence number `irs` and the
/// handshake numbers advanced by hand. Our ISS is `iss`; nothing has been sent
/// or acknowledged beyond what `state` implies.
pub fn connection(state: State, sport: u16, dport: u16, iss: u32, irs: u32) -> Connection {
    let established = !matches!(state, State::SynRcvd | State::Listen | State::Closed);
    let tcp = TcpHeader {
        source_port: dport,
        destination_port: sport,
        sequence_number: iss,
        acknowledgment_number: irs.wrapping_add(1),
        ns: false,
        cwr: false,
        ece: false,
        urg: false,
        ack: true,
        psh: false,
        rst: false,
        syn: false,
        fin: false,
        window_size: 10,
        checksum: 0,
        urgent_pointer: 0,
        options: Vec::new(),
    };
    Connection {
        state,
        send: SendSequenceSpace {
            una: if established { iss.wrapping_add(1) } else { iss },
            nxt: iss.wrapping_add(1),
            wnd: 10,
            up: false,
            wl1: 0,
            wl2: 0,
            iss,
        },
        recv: RecvSequenceSpace { nxt: irs.wrapping_add(1), wnd: 64240, up: false, irs },
        ip: Ipv4Header {
            dscp_ecn: 0,
            total_len: 40,
            identification: 0,
            dont_fragment: true,
            more_fragments: false,
            fragment_offset: 0,
            ttl: 64,
            protocol: 6,
            header_checksum: 0,
            source: SERVER,
            destination: CLIENT,
            options: Vec::new(),
        },
        tcp,
        unacked: VecDeque::new(),
        closed_at: None,
    }
}
