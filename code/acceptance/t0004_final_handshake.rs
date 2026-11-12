//! Acceptance test for task 0004-final-handshake (chapter 4). Outside every task's write scope.
//!
//! Connections are built field by field (acceptance/common `connection`), not
//! through `accept`, so this task does not depend on chapter 3's code. Sent
//! segments are decoded by the test's own codec.
//!
//! Deliberately NOT tested, because the book calls them loose ends that later
//! chapters change: a FIN received in `Estab`, a FIN whose ACK number is not
//! new (the chapter 4 Aside), what `send_rst` puts in its numbers, what happens
//! after `TimeWait`, whether data segments are acknowledged, and `close` in any
//! state but `Estab`.

mod common;

use std::io;

use common::{connection, RecordingNic, Seg, ACK, FIN, PSH, RST, SYN};
use tcp::tcp::{is_between_wrapped, wrapping_lt, Connection, State};
use tcp::Nic;

const CPORT: u16 = 51234;
const SPORT: u16 = 80;

fn deliver(c: &mut Connection, nic: &mut RecordingNic, seg: &Seg) {
    let (iph, tcph) = seg.headers();
    c.on_packet(nic, &iph, &tcph, &seg.payload).expect("on_packet must not fail on a RecordingNic");
}

fn from_client(seq: u32, ack: u32, flags: u8) -> Seg {
    Seg::client(CPORT, SPORT, seq, ack, flags)
}

// ---------------------------------------------------------------- seq.rs

#[test]
fn wrapping_less_than() {
    assert!(wrapping_lt(1, 2));
    assert!(!wrapping_lt(2, 1));
    assert!(!wrapping_lt(5, 5), "strict");
    assert!(wrapping_lt(u32::MAX, 0), "0 comes right after u32::MAX");
    assert!(!wrapping_lt(0, u32::MAX));
    assert!(wrapping_lt(u32::MAX - 10, 5));
    assert!(wrapping_lt(0, 1 << 30));
    assert!(!wrapping_lt(1 << 30, 0));
    assert!(!wrapping_lt(0, 1 << 31), "exactly half way round, neither comes first");
    assert!(!wrapping_lt(1 << 31, 0), "exactly half way round, neither comes first");
    assert!(wrapping_lt(1, 1 << 31), "just under half way");
}

#[test]
fn between_wrapped() {
    assert!(is_between_wrapped(1, 2, 3));
    assert!(!is_between_wrapped(1, 1, 3), "start is exclusive");
    assert!(!is_between_wrapped(1, 3, 3), "end is exclusive");
    assert!(!is_between_wrapped(10, 5, 20));
    assert!(!is_between_wrapped(10, 25, 20));
    assert!(is_between_wrapped(u32::MAX - 1, u32::MAX, 0));
    assert!(is_between_wrapped(u32::MAX, 0, 1));
    assert!(is_between_wrapped(u32::MAX - 5, 3, 10), "across the wrap");
}

// ---------------------------------------------------------------- write.rs

#[test]
fn write_an_empty_ack() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    let sent = c.write(&mut nic, c.send.nxt, 0).unwrap();
    assert_eq!(sent, 0);
    let seg = nic.take_one();
    assert_eq!((seg.sport, seg.dport), (SPORT, CPORT));
    assert_eq!(seg.seq, 1, "the sequence number we were given");
    assert_eq!(seg.ack, 1001, "always acknowledges recv.nxt");
    assert_eq!(seg.flags, ACK);
    assert_eq!(seg.window, 10);
    assert!(seg.payload.is_empty());
    assert_eq!(c.send.nxt, 1, "an empty ACK consumes no sequence space");
    assert_eq!(c.tcp.sequence_number, 1, "the stored header is updated, as in the book");
    assert_eq!(c.tcp.acknowledgment_number, 1001);
}

#[test]
fn write_sends_from_unacked() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked.extend(b"hello world");

    assert_eq!(c.write(&mut nic, 1, 5).unwrap(), 5, "limit caps the payload");
    let seg = nic.take_one();
    assert_eq!(seg.payload, b"hello");
    assert_eq!(seg.seq, 1);
    assert_eq!(c.send.nxt, 6);

    assert_eq!(c.write(&mut nic, 6, 100).unwrap(), 6, "the rest, from offset seq - una");
    let seg = nic.take_one();
    assert_eq!(seg.payload, b" world");
    assert_eq!(seg.seq, 6);
    assert_eq!(c.send.nxt, 12);
    assert_eq!(c.unacked.len(), 11, "write does not remove anything from unacked (only an ACK does, later)");

    assert_eq!(c.write(&mut nic, 1, 2).unwrap(), 2, "a retransmission");
    assert_eq!(nic.take_one().payload, b"he");
    assert_eq!(c.send.nxt, 12, "send.nxt never moves backwards");
}

#[test]
fn write_handles_a_wrapped_deque() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked = std::collections::VecDeque::with_capacity(8);
    c.unacked.extend(b"012345");
    for _ in 0..4 {
        c.unacked.pop_front();
    }
    c.unacked.extend(b"6789ab");
    let (head, tail) = c.unacked.as_slices();
    let want: Vec<u8> = head.iter().chain(tail).copied().collect();
    assert_eq!(want, b"456789ab");

    assert_eq!(c.write(&mut nic, 1, 100).unwrap(), 8);
    assert_eq!(nic.take_one().payload, b"456789ab", "both halves of the ring buffer, in order");
    assert_eq!(c.write(&mut nic, 4, 3).unwrap(), 3);
    assert_eq!(nic.take_one().payload, b"789", "an offset that lands in the second half");
}

#[test]
fn write_respects_the_mtu() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked.extend(std::iter::repeat(b'x').take(3000));
    assert_eq!(c.write(&mut nic, 1, 5000).unwrap(), 1460, "1500 bytes minus 20 + 20 of headers");
    assert_eq!(nic.sent[0].len(), 1500);
    assert_eq!(nic.take_one().payload.len(), 1460);
    assert_eq!(c.send.nxt, 1461);
}

#[test]
fn write_past_the_end_of_unacked_sends_no_data() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked.extend(b"ab");
    assert_eq!(c.write(&mut nic, 6, 10).unwrap(), 0, "offset 5 is beyond the 2 queued bytes: no panic, no data");
    assert!(nic.take_one().payload.is_empty());
}

#[test]
fn syn_and_fin_consume_a_sequence_number_and_are_sent_once() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::SynRcvd, CPORT, SPORT, 500, 1000);
    c.send.nxt = 500;
    c.tcp.syn = true;
    c.write(&mut nic, 500, 0).unwrap();
    let seg = nic.take_one();
    assert_eq!(seg.flags, SYN | ACK);
    assert_eq!(seg.seq, 500);
    assert!(!c.tcp.syn, "the SYN flag is cleared after it has been sent");
    assert_eq!(c.send.nxt, 501);

    let mut c = connection(State::Estab, CPORT, SPORT, 500, 1000);
    c.tcp.fin = true;
    c.write(&mut nic, 501, 0).unwrap();
    let seg = nic.take_one();
    assert_eq!(seg.flags, FIN | ACK);
    assert!(!c.tcp.fin, "the FIN flag is cleared after it has been sent");
    assert_eq!(c.send.nxt, 502);
    c.write(&mut nic, 502, 0).unwrap();
    assert_eq!(nic.take_one().flags, ACK, "the next segment is a plain ACK");

    let mut c = connection(State::Estab, CPORT, SPORT, 500, 1000);
    c.unacked.extend(b"bye");
    c.tcp.fin = true;
    assert_eq!(c.write(&mut nic, 501, 10).unwrap(), 3);
    let seg = nic.take_one();
    assert_eq!((seg.flags, seg.payload.as_slice()), (FIN | ACK, &b"bye"[..]));
    assert_eq!(c.send.nxt, 505, "3 bytes of data, then the FIN");
}

#[test]
fn nothing_is_sent_after_our_fin() {
    // The book's rule: a segment that starts right after the FIN carries no data.
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked.extend(b"xyz");
    c.closed_at = Some(0);
    assert_eq!(c.write(&mut nic, 1, 10).unwrap(), 0);
    assert!(nic.take_one().payload.is_empty());
    c.closed_at = None;
    assert_eq!(c.write(&mut nic, 1, 10).unwrap(), 3, "without closed_at the same call sends the data");
}

#[test]
fn write_across_the_wrap() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, u32::MAX, 1000);
    assert_eq!((c.send.una, c.send.nxt), (0, 0));
    c.unacked.extend(b"abc");
    c.write(&mut nic, u32::MAX.wrapping_add(1), 10).unwrap();
    assert_eq!(c.send.nxt, 3);
    let mut c = connection(State::Estab, CPORT, SPORT, u32::MAX - 1, 1000);
    c.unacked.extend(b"abc");
    c.write(&mut nic, u32::MAX, 10).unwrap();
    let seg = nic.take();
    assert_eq!(seg[1].seq, u32::MAX);
    assert_eq!(seg[1].payload, b"abc");
    assert_eq!(c.send.nxt, 2, "u32::MAX + 3 wraps to 2");
}

// ---------------------------------------------------------------- on_packet.rs

/// The capture from the "Testing It Out" section of chapter 4, with chapter
/// 5's change: the stack no longer hangs up by itself, the application calls
/// `close` (here: the test).
#[test]
fn the_book_capture_from_handshake_to_time_wait() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::SynRcvd, CPORT, SPORT, 0, 1000);
    let iss = c.send.iss;

    // 3. the client's ACK completes the handshake
    deliver(&mut c, &mut nic, &from_client(1001, iss.wrapping_add(1), ACK));
    assert_eq!(c.state, State::Estab);
    assert!(c.is_established());
    assert_eq!(c.send.una, iss.wrapping_add(1), "the ACK acknowledged our SYN");
    assert_eq!(c.recv.nxt, 1001);
    assert!(nic.sent.is_empty(), "no reply to the handshake ACK, and no FIN: chapter 5 removed the auto hang-up");

    // 4. we close: FIN, ACK
    c.close(&mut nic).unwrap();
    let fin = nic.take_one();
    assert_eq!(fin.flags, FIN | ACK);
    assert_eq!(fin.seq, iss.wrapping_add(1));
    assert_eq!(fin.ack, 1001);
    assert_eq!(c.state, State::FinWait1);
    assert!(!c.is_established());
    assert_eq!(c.send.nxt, iss.wrapping_add(2), "our FIN consumed a sequence number");
    assert_eq!(c.closed_at, Some(iss.wrapping_add(1)), "closed_at is the sequence number of our FIN");

    // 5. the client acknowledges our FIN and sends its own in one segment
    deliver(&mut c, &mut nic, &from_client(1001, iss.wrapping_add(2), FIN | ACK));
    assert_eq!(c.recv.nxt, 1002, "the client's FIN consumed a sequence number");
    assert_eq!(c.send.una, iss.wrapping_add(2));

    // 6. our last ACK
    let last = nic.take_one();
    assert_eq!(last.flags, ACK);
    assert_eq!(last.seq, iss.wrapping_add(2));
    assert_eq!(last.ack, 1002);
    assert_eq!(c.state, State::TimeWait);
}

#[test]
fn a_bare_ack_of_our_fin_moves_to_fin_wait_2() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.close(&mut nic).unwrap();
    nic.take_one();
    deliver(&mut c, &mut nic, &from_client(1001, 2, ACK));
    assert_eq!(c.state, State::FinWait2);
    assert_eq!(c.send.una, 2);
    assert!(nic.sent.is_empty(), "a bare ACK needs no reply");
}

#[test]
fn an_ack_that_does_not_cover_our_fin_keeps_fin_wait_1() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.unacked.extend(b"ab");
    c.write(&mut nic, 1, 2).unwrap();
    c.close(&mut nic).unwrap();
    nic.take();
    assert_eq!(c.send.nxt, 4);
    deliver(&mut c, &mut nic, &from_client(1001, 3, ACK));
    assert_eq!(c.send.una, 3);
    assert_eq!(c.state, State::FinWait1, "send.una is not iss + 2");
}

#[test]
fn the_handshake_across_the_wrap() {
    let mut nic = RecordingNic::new();
    let iss = u32::MAX;
    let irs = u32::MAX - 1;
    let mut c = connection(State::SynRcvd, CPORT, SPORT, iss, irs);
    assert_eq!((c.send.una, c.send.nxt, c.recv.nxt), (u32::MAX, 0, u32::MAX));

    deliver(&mut c, &mut nic, &from_client(u32::MAX, 0, ACK));
    assert_eq!(c.state, State::Estab);
    assert_eq!(c.send.una, 0);

    c.close(&mut nic).unwrap();
    assert_eq!(nic.take_one().seq, 0);
    deliver(&mut c, &mut nic, &from_client(u32::MAX, 1, FIN | ACK));
    assert_eq!(c.recv.nxt, 0, "u32::MAX + 1 wraps");
    assert_eq!(c.state, State::TimeWait, "send.una == iss + 2 holds modulo 2^32");
    let last = nic.take_one();
    assert_eq!((last.seq, last.ack, last.flags), (1, 0, ACK));
}

#[test]
fn an_unacceptable_segment_gets_an_ack_and_changes_nothing() {
    for seg in [
        from_client(1001 + 70_000, 1, ACK).with_payload(b"far ahead"),
        from_client(1001 + 70_000, 1, ACK),
        from_client(900, 1, ACK),
        from_client(900, 1, ACK | PSH).with_payload(b"old"),
    ] {
        let mut nic = RecordingNic::new();
        let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
        deliver(&mut c, &mut nic, &seg);
        let reply = nic.take_one();
        assert_eq!(reply.flags, ACK, "for {:?}", seg);
        assert_eq!(reply.seq, c.send.nxt);
        assert_eq!(reply.ack, 1001, "we still expect 1001");
        assert_eq!(c.recv.nxt, 1001);
        assert_eq!(c.state, State::Estab);
        assert_eq!(c.send.una, 1);
    }
}

#[test]
fn a_duplicate_ack_is_accepted_quietly() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    deliver(&mut c, &mut nic, &from_client(1001, 1, ACK));
    assert!(nic.sent.is_empty());
    assert_eq!(c.state, State::Estab);
    assert_eq!(c.send.una, 1);
}

#[test]
fn a_segment_without_ack_only_moves_recv_nxt() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::SynRcvd, CPORT, SPORT, 0, 1000);
    deliver(&mut c, &mut nic, &from_client(1001, 0, 0));
    assert_eq!(c.state, State::SynRcvd, "no ACK, no handshake");
    assert_eq!(c.send.una, 0);
    assert!(nic.sent.is_empty());
}

#[test]
fn a_wrong_ack_in_syn_rcvd_does_not_establish() {
    // RFC 793 wants a reset here; that is a loose end, so what is sent is not checked.
    for ackn in [5, 0x8000_0000, 2] {
        let mut nic = RecordingNic::new();
        let mut c = connection(State::SynRcvd, CPORT, SPORT, 0, 1000);
        deliver(&mut c, &mut nic, &from_client(1001, ackn, ACK));
        assert_eq!(c.state, State::SynRcvd, "ack {} does not acknowledge our SYN", ackn);
    }
}

#[test]
fn a_zero_receive_window() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.recv.wnd = 0;
    deliver(&mut c, &mut nic, &from_client(1001, 1, ACK));
    assert!(nic.sent.is_empty(), "a zero-length segment exactly at recv.nxt is acceptable");

    deliver(&mut c, &mut nic, &from_client(1002, 1, ACK));
    assert_eq!(nic.take_one().ack, 1001, "anywhere else it is not");

    deliver(&mut c, &mut nic, &from_client(1001, 1, ACK | PSH).with_payload(b"x"));
    assert_eq!(nic.take_one().ack, 1001, "no data fits in a closed window");
    assert_eq!(c.recv.nxt, 1001);
}

#[test]
fn data_does_not_crash_the_stack() {
    // The book asserts that data is empty; a library must not panic on what
    // a peer sends. The payload is not stored until chapter 6.
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    deliver(&mut c, &mut nic, &from_client(1001, 1, ACK | PSH).with_payload(b"hello"));
    assert_eq!(c.recv.nxt, 1006);
    assert_eq!(c.state, State::Estab);

    // Partly old, partly new: acceptable because its last byte is in the window.
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    deliver(&mut c, &mut nic, &from_client(999, 1, ACK | PSH).with_payload(b"abcde"));
    assert_eq!(c.recv.nxt, 1004, "recv.nxt = seq + len, as in the book");
}

#[test]
fn is_established_only_in_estab() {
    for (state, want) in [
        (State::SynRcvd, false),
        (State::Estab, true),
        (State::FinWait1, false),
        (State::FinWait2, false),
        (State::TimeWait, false),
        (State::Listen, false),
        (State::Closed, false),
    ] {
        assert_eq!(connection(state, CPORT, SPORT, 0, 1000).is_established(), want, "{:?}", state);
    }
}

#[test]
fn send_rst_sends_one_reset() {
    let mut nic = RecordingNic::new();
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    c.send_rst(&mut nic).unwrap();
    let seg = nic.take_one();
    assert!(seg.has(RST), "the RST flag is set");
    assert!(seg.payload.is_empty());
}

#[test]
fn nic_errors_are_returned_not_panicked() {
    struct BrokenNic;
    impl Nic for BrokenNic {
        fn recv(&mut self, _: &mut [u8]) -> io::Result<usize> {
            unreachable!()
        }
        fn send(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("cable unplugged"))
        }
    }
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    assert!(c.write(&mut BrokenNic, 1, 0).is_err());
    assert!(c.close(&mut BrokenNic).is_err());
    let mut c = connection(State::Estab, CPORT, SPORT, 0, 1000);
    let (iph, tcph) = from_client(5_000_000, 1, ACK).headers();
    assert!(c.on_packet(&mut BrokenNic, &iph, &tcph, &[]).is_err(), "the ACK for an unacceptable segment fails");
}
