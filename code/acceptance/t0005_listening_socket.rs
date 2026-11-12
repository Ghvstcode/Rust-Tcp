//! Acceptance test for task 0005-listening-socket (chapter 5). Outside every task's write scope.
//!
//! The real packet loop runs on its own thread against a channel-backed NIC
//! (acceptance/common `channel_nic`). Every wait has a timeout, so a broken
//! implementation fails in seconds rather than hanging.
//!
//! Deliberately NOT tested, because the book calls them loose ends: what is
//! sent in reply to a SYN for a port nobody listens on (nothing today, a RST
//! later), what happens to connections when their listener goes away, and
//! what dropping a stream or the interface sends.

mod common;

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use common::{channel_nic, within, within_for, Seg, Wire, ACK, CLIENT, PSH, SERVER, SYN};
use tcp::{Interface, TcpListener, TcpStream};

fn client_addr(port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::from(CLIENT), port)
}

fn server_addr(port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::from(SERVER), port)
}

fn setup() -> (Interface, Wire) {
    let (nic, wire) = channel_nic();
    (Interface::with_nic(nic), wire)
}

/// Accept on another thread, with a deadline.
fn accept_soon(listener: TcpListener) -> (TcpStream, SocketAddrV4, TcpListener) {
    within(move || {
        let (s, a) = listener.accept().expect("accept returned an error");
        (s, a, listener)
    })
    .expect("accept did not return a connection within 5 seconds")
}

#[test]
fn bind_twice_is_addr_in_use() {
    let (iface, _wire) = setup();
    let l = iface.bind(8000).expect("first bind");
    let err = iface.bind(8000).err().expect("second bind of the same port must fail");
    assert_eq!(err.kind(), ErrorKind::AddrInUse);
    let _other = iface.bind(9000).expect("a different port is fine");
    drop(l);
    assert!(iface.bind(8000).is_ok(), "dropping the listener unbinds the port");
}

#[test]
fn accept_hands_over_an_established_connection() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();

    wire.handshake(51234, 8000, 1000);
    let (stream, peer, _l) = accept_soon(listener);
    assert_eq!(peer, client_addr(51234));
    assert_eq!(stream.peer_addr(), client_addr(51234));
    assert_eq!(stream.local_addr(), server_addr(8000));
}

#[test]
fn the_syn_ack_comes_from_the_packet_loop() {
    let (iface, mut wire) = setup();
    let _l = iface.bind(8000).unwrap();
    wire.send(&Seg::client(40001, 8000, 4242, 0, SYN));
    let syn_ack = wire.expect_syn_ack(40001, 8000);
    assert_eq!(syn_ack.ack, 4243);
    assert_eq!(syn_ack.src, SERVER);
    assert_eq!(syn_ack.dst, CLIENT);
}

#[test]
fn no_handover_before_the_handshake_completes() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    wire.send(&Seg::client(51234, 8000, 1000, 0, SYN));
    let syn_ack = wire.expect_syn_ack(51234, 8000);

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(listener.accept().map(|(_, a)| a).ok());
        drop(listener);
    });
    let early = rx.recv_timeout(Duration::from_millis(500));
    assert!(early.is_err(), "accept must wait for the client's ACK, it returned {:?}", early);

    // Now the ACK: the same, still blocked, accept gets the connection.
    wire.send(&Seg::client(51234, 8000, 1001, syn_ack.seq.wrapping_add(1), ACK));
    let got = rx.recv_timeout(common::PATIENCE).expect("the handshake completed but accept never returned");
    assert_eq!(got, Some(client_addr(51234)));
}

#[test]
fn a_blocked_accept_wakes_up_when_a_connection_arrives() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = listener.accept().map(|(s, a)| (s.local_addr(), a));
        let _ = tx.send(r.ok());
        drop(listener);
    });
    std::thread::sleep(Duration::from_millis(200)); // let it block first
    wire.handshake(51234, 8000, 1000);
    let got = rx.recv_timeout(common::PATIENCE).expect("the blocked accept was never woken up");
    assert_eq!(got, Some((server_addr(8000), client_addr(51234))));
}

#[test]
fn syns_for_unbound_ports_create_nothing() {
    let (iface, mut wire) = setup();
    let l = iface.bind(8000).unwrap();
    wire.send(&Seg::client(51234, 7000, 1000, 0, SYN));
    // The NIC is a FIFO: once the SYN-ACK for this later SYN to port 8000 is
    // out, the packet loop has certainly handled the SYN to port 7000, while
    // 7000 was still unbound.
    wire.send(&Seg::client(51236, 8000, 1, 0, SYN));
    let syn_ack = wire.expect_syn_ack(51236, 8000);

    // Whatever was or was not sent back, no connection exists for port 7000:
    // binding it now and finishing that old handshake must not hand one over.
    let listener = iface.bind(7000).unwrap();
    wire.send(&Seg::client(51234, 7000, 1001, 1, ACK));
    let pending = within_for(Duration::from_millis(500), move || listener.accept().map(|(_, a)| a).ok());
    assert!(pending.is_none(), "no handshake was ever started for port 7000, yet accept returned {:?}", pending);

    // ...and the loop is alive and well for ports that are bound.
    wire.send(&Seg::client(51236, 8000, 2, syn_ack.seq.wrapping_add(1), ACK));
    let (_, peer, _l) = accept_soon(l);
    assert_eq!(peer, client_addr(51236));
}

#[test]
fn two_ports_two_listeners() {
    let (iface, mut wire) = setup();
    let a = iface.bind(8000).unwrap();
    let b = iface.bind(9000).unwrap();
    wire.handshake(40000, 9000, 100);
    wire.handshake(40001, 8000, 200);
    let (sb, pb, _b) = accept_soon(b);
    let (sa, pa, _a) = accept_soon(a);
    assert_eq!((pa, sa.local_addr()), (client_addr(40001), server_addr(8000)));
    assert_eq!((pb, sb.local_addr()), (client_addr(40000), server_addr(9000)));
}

#[test]
fn the_backlog_is_first_in_first_out() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    wire.handshake(50001, 8000, 1);
    wire.handshake(50002, 8000, 2);
    wire.handshake(50003, 8000, 3);
    let (_, p1, listener) = accept_soon(listener);
    let (_, p2, listener) = accept_soon(listener);
    let (_, p3, _listener) = accept_soon(listener);
    assert_eq!([p1.port(), p2.port(), p3.port()], [50001, 50002, 50003]);
}

#[test]
fn many_connections_from_one_client_address() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    let mut listener = Some(listener);
    for port in 30000..30020u16 {
        wire.handshake(port, 8000, u32::from(port) * 7);
        let (s, p, l) = accept_soon(listener.take().unwrap());
        assert_eq!(p.port(), port, "each quad is its own connection");
        assert_eq!(s.peer_addr(), client_addr(port));
        listener = Some(l);
    }
}

#[test]
fn garbage_does_not_stop_the_packet_loop() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    wire.inject(vec![]);
    wire.inject(vec![0x45]);
    wire.inject(vec![0x60; 60]); // IPv6
    let mut udp = Seg::client(1, 8000, 0, 0, SYN).encode();
    udp[9] = 17; // claims to be UDP
    wire.inject(udp);
    let mut short_tcp = Seg::client(1, 8000, 0, 0, SYN).encode();
    short_tcp.truncate(30); // IPv4 header, half a TCP header
    wire.inject(short_tcp);
    let mut bad_offset = Seg::client(1, 8000, 0, 0, SYN).encode();
    bad_offset[32] = 0x10; // TCP data offset 1
    wire.inject(bad_offset);

    wire.handshake(51234, 8000, 99);
    let (_, peer, _l) = accept_soon(listener);
    assert_eq!(peer, client_addr(51234));
}

#[test]
fn data_on_an_established_connection_does_not_kill_the_loop() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    let server_isn = wire.handshake(51234, 8000, 1000);
    let (_stream, _, listener) = accept_soon(listener);
    wire.send(&Seg::client(51234, 8000, 1001, server_isn.wrapping_add(1), ACK | PSH).with_payload(b"hello\n"));

    wire.handshake(51235, 8000, 5000);
    let (_, peer, _l) = accept_soon(listener);
    assert_eq!(peer, client_addr(51235), "the loop survived the data and handled the next client");
}

#[test]
fn a_syn_for_an_existing_connection_is_not_a_new_one() {
    let (iface, mut wire) = setup();
    let listener = iface.bind(8000).unwrap();
    wire.handshake(51234, 8000, 1000);
    let (_s, _, listener) = accept_soon(listener);
    // A stray retransmitted SYN on the same quad goes to that connection's
    // on_packet, not to a second handshake.
    wire.send(&Seg::client(51234, 8000, 1000, 0, SYN));
    let pending = within_for(Duration::from_millis(500), move || listener.accept().map(|(_, a)| a).ok());
    assert!(pending.is_none(), "the same quad was handed over twice: {:?}", pending);
}

#[test]
fn types_can_move_between_threads() {
    fn assert_send<T: Send + 'static>() {}
    assert_send::<Interface>();
    assert_send::<TcpListener>();
    assert_send::<TcpStream>();
}
