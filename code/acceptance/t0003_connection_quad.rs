//! Acceptance test for task 0003-connection-quad (chapter 3). Outside every task's write scope.
//!
//! `Connection::accept` is called directly with hand-built headers, and the
//! SYN-ACK it sends is decoded by the test's own codec (acceptance/common),
//! which checks both checksums and every length a real peer would check.

mod common;

use std::io;

use common::{RecordingNic, Seg, ACK, CLIENT, FIN, PSH, RST, SERVER, SYN};
use tcp::tcp::{Connection, State};
use tcp::Nic;

fn accept(nic: &mut RecordingNic, seg: &Seg) -> Option<Connection> {
    let (iph, tcph) = seg.headers();
    Connection::accept(nic, &iph, &tcph, &seg.payload).expect("accept must not fail on a RecordingNic")
}

#[test]
fn a_syn_starts_a_handshake() {
    let mut nic = RecordingNic::new();
    let syn = Seg::client(51234, 80, 1000, 0, SYN);
    let c = accept(&mut nic, &syn).expect("a SYN creates a connection");

    assert_eq!(c.state, State::SynRcvd);
    assert_eq!(c.recv.irs, 1000, "IRS is the client's sequence number");
    assert_eq!(c.recv.nxt, 1001, "the SYN consumes one sequence number");
    assert_eq!(c.recv.wnd, 64240, "as in the book, recv.wnd is taken from the SYN's window");
    assert_eq!(c.send.una, c.send.iss, "nothing of ours is acknowledged yet");
    assert_eq!(c.send.nxt, c.send.iss.wrapping_add(1), "our SYN consumed one sequence number");
    assert_eq!(c.send.wnd, 10, "the book's tiny hard-coded window");
    assert!(c.unacked.is_empty());
    assert_eq!(c.closed_at, None);
}

#[test]
fn the_syn_ack_on_the_wire() {
    let mut nic = RecordingNic::new();
    let syn = Seg::client(51234, 80, 1000, 0, SYN);
    let c = accept(&mut nic, &syn).unwrap();
    let out = nic.take_one(); // decode() checks checksums, lengths, version and protocol

    assert_eq!(out.src, SERVER, "the SYN-ACK comes from the address the client dialled");
    assert_eq!(out.dst, CLIENT);
    assert_eq!(out.sport, 80, "ports are swapped");
    assert_eq!(out.dport, 51234);
    assert_eq!(out.seq, c.send.iss, "Seq is our ISS");
    assert_eq!(out.ack, 1001, "Ack is the client's ISN + 1");
    assert_eq!(out.flags, SYN | ACK, "SYN and ACK, nothing else");
    assert_eq!(out.window, c.send.wnd);
    assert_eq!(out.window, 10);
    assert!(out.payload.is_empty());

    let ip = &nic_ip_bytes(&syn);
    assert_eq!(ip[8], 64, "TTL 64, as in the book");
}

/// Re-run accept and return the raw bytes of the datagram, for header fields
/// `Seg` does not keep.
fn nic_ip_bytes(syn: &Seg) -> Vec<u8> {
    let mut nic = RecordingNic::new();
    accept(&mut nic, syn).unwrap();
    assert_eq!(nic.sent.len(), 1);
    let d = nic.sent.remove(0);
    assert_eq!(d.len(), 40, "20 bytes of IPv4 header and 20 of TCP header, no options, no payload");
    d
}

#[test]
fn the_connection_keeps_its_headers_for_later_segments() {
    let mut nic = RecordingNic::new();
    let c = accept(&mut nic, &Seg::client(40000, 8000, 77, 0, SYN)).unwrap();
    assert_eq!(c.ip.source, SERVER);
    assert_eq!(c.ip.destination, CLIENT);
    assert_eq!(c.ip.protocol, 6);
    assert_eq!(c.ip.ttl, 64);
    assert_eq!(c.tcp.source_port, 8000);
    assert_eq!(c.tcp.destination_port, 40000);
    assert!(c.tcp.ack, "every later segment carries ACK");
    assert!(!c.tcp.syn, "the SYN flag is cleared once the SYN-ACK is out, so it is never sent twice");
    assert!(!c.tcp.fin && !c.tcp.rst);
    assert_eq!(c.tcp.window_size, 10);
    assert_eq!(c.tcp.acknowledgment_number, 78);
}

#[test]
fn anything_but_a_syn_is_ignored() {
    for flags in [ACK, FIN, FIN | ACK, PSH | ACK, 0] {
        let mut nic = RecordingNic::new();
        let seg = Seg::client(51234, 80, 1000, 5, flags).with_payload(b"data");
        assert!(accept(&mut nic, &seg).is_none(), "flags {:#04x} must not create a connection", flags);
        assert!(nic.sent.is_empty(), "and nothing is sent in reply (flags {:#04x})", flags);
    }
}

#[test]
fn sequence_numbers_wrap() {
    let mut nic = RecordingNic::new();
    let c = accept(&mut nic, &Seg::client(51234, 80, u32::MAX, 0, SYN)).unwrap();
    assert_eq!(c.recv.irs, u32::MAX);
    assert_eq!(c.recv.nxt, 0, "u32::MAX + 1 wraps to 0");
    assert_eq!(nic.take_one().ack, 0);
}

#[test]
fn many_connections_one_syn_ack_each() {
    let mut nic = RecordingNic::new();
    let mut conns = Vec::new();
    for (i, port) in [50000u16, 50001, 50002].iter().enumerate() {
        let isn = 10_000 * (i as u32 + 1);
        conns.push(accept(&mut nic, &Seg::client(*port, 9000, isn, 0, SYN)).unwrap());
    }
    let out = nic.take();
    assert_eq!(out.len(), 3);
    for (i, seg) in out.iter().enumerate() {
        assert_eq!(seg.dport, [50000, 50001, 50002][i]);
        assert_eq!(seg.ack, 10_000 * (i as u32 + 1) + 1);
        assert_eq!(seg.flags, SYN | ACK);
    }
}

#[test]
fn the_syn_ack_is_one_send_call() {
    // A TUN device takes exactly one datagram per write: header and all.
    struct CountingNic(usize);
    impl Nic for CountingNic {
        fn recv(&mut self, _: &mut [u8]) -> io::Result<usize> {
            unreachable!()
        }
        fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0 += 1;
            Ok(buf.len())
        }
    }
    let mut nic = CountingNic(0);
    let (iph, tcph) = Seg::client(1, 2, 3, 0, SYN).headers();
    Connection::accept(&mut nic, &iph, &tcph, &[]).unwrap().unwrap();
    assert_eq!(nic.0, 1);
}

#[test]
fn a_failing_nic_is_an_error_not_a_panic() {
    struct BrokenNic;
    impl Nic for BrokenNic {
        fn recv(&mut self, _: &mut [u8]) -> io::Result<usize> {
            unreachable!()
        }
        fn send(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("cable unplugged"))
        }
    }
    let (iph, tcph) = Seg::client(1, 2, 3, 0, SYN).headers();
    let err = Connection::accept(&mut BrokenNic, &iph, &tcph, &[]).expect_err("the send error is returned");
    assert_eq!(err.to_string(), "cable unplugged");
}

#[test]
fn odd_flag_combinations_do_not_panic() {
    // Whether a SYN that also carries RST starts a handshake is left open (the
    // book only looks at the SYN bit; a later chapter may not). This only
    // checks that nothing panics.
    let mut nic = RecordingNic::new();
    let _ = accept(&mut nic, &Seg::client(1, 2, 3, 0, SYN | RST));
}
