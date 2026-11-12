//! Acceptance test for task 0002-parsing-the-bits (chapter 2). Outside every task's write scope.
//!
//! The byte vectors below are real datagrams: the IPv4 header is the worked
//! example from Wikipedia's "IPv4 header checksum" article, the SYN and the
//! data segment were built by a separate script (Python's struct module), and
//! the TUN bytes are the ones chapter 1 prints for a `ping`.

mod common;

use std::io::ErrorKind;
use std::net::Ipv4Addr;

use tcp::packet::{
    internet_checksum, Ipv4Header, PacketError, PacketInfo, TcpHeader, ETHERTYPE_IPV4, ETHERTYPE_IPV6, PROTO_TCP,
};

/// Wikipedia's example: UDP, 192.168.0.1 -> 192.168.0.199, checksum 0xb861.
const WIKI_IPV4: [u8; 20] = [
    0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8, 0x00, 0x01, 0xc0, 0xa8,
    0x00, 0xc7,
];

/// 192.168.0.1:51234 -> 192.168.0.2:80, SYN, seq 0x1a2b3c4d, window 64240, MSS 1460 option.
const SYN: [u8; 44] = [
    0x45, 0x00, 0x00, 0x2c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x9d, 0x32, 0xc0, 0xa8, 0x00, 0x01, 0xc0, 0xa8,
    0x00, 0x02, 0xc8, 0x22, 0x00, 0x50, 0x1a, 0x2b, 0x3c, 0x4d, 0x00, 0x00, 0x00, 0x00, 0x60, 0x02, 0xfa, 0xf0,
    0xfc, 0xf6, 0x00, 0x00, 0x02, 0x04, 0x05, 0xb4,
];

/// Same connection, PSH+ACK, seq 0x1a2b3c4e, ack 1, window 502, payload "hello" (odd length).
const DATA: [u8; 45] = [
    0x45, 0x00, 0x00, 0x2d, 0x1c, 0x47, 0x40, 0x00, 0x40, 0x06, 0x9d, 0x30, 0xc0, 0xa8, 0x00, 0x01, 0xc0, 0xa8,
    0x00, 0x02, 0xc8, 0x22, 0x00, 0x50, 0x1a, 0x2b, 0x3c, 0x4e, 0x00, 0x00, 0x00, 0x01, 0x50, 0x18, 0x01, 0xf6,
    0xc9, 0xbe, 0x00, 0x00, 0x68, 0x65, 0x6c, 0x6c, 0x6f,
];

/// The start of what chapter 1 reads from tun0 for a ping (packet info, then IPv6).
const TUN_PING: [u8; 12] = [0x00, 0x00, 0x86, 0xdd, 0x60, 0x00, 0x00, 0x00, 0x00, 0x08, 0x3a, 0xff];

fn bytes_of_ip(h: &Ipv4Header) -> Vec<u8> {
    let mut out = Vec::new();
    h.write(&mut out).expect("writing to a Vec cannot fail");
    out
}

fn bytes_of_tcp(h: &TcpHeader) -> Vec<u8> {
    let mut out = Vec::new();
    h.write(&mut out).expect("writing to a Vec cannot fail");
    out
}

// ---------------------------------------------------------------- checksum

#[test]
fn internet_checksum_rfc1071() {
    assert_eq!(internet_checksum(&[0x00, 0x01, 0xf2, 0x03, 0xf4, 0xf5, 0xf6, 0xf7]), 0x220d, "RFC 1071 section 3");
    assert_eq!(internet_checksum(&[0x01]), 0xfeff, "an odd trailing byte is padded with a zero byte");
    assert_eq!(internet_checksum(&[]), 0xffff);
    assert_eq!(internet_checksum(&[0xff, 0xff, 0xff, 0xff, 0x00, 0x01]), 0xfffe, "carries fold back in");
    let mut zeroed = WIKI_IPV4;
    zeroed[10] = 0;
    zeroed[11] = 0;
    assert_eq!(internet_checksum(&zeroed), 0xb861);
    assert_eq!(internet_checksum(&WIKI_IPV4), 0, "a header with a correct checksum sums to zero");
}

#[test]
fn internet_checksum_handles_large_input() {
    let big = vec![0xffu8; 70_000];
    assert_eq!(internet_checksum(&big), common::checksum(&big));
}

// ---------------------------------------------------------------- packet info

#[test]
fn packet_info_from_chapter_one() {
    let (info, rest) = PacketInfo::parse(&TUN_PING).expect("4 bytes of packet info");
    assert_eq!(info, PacketInfo { flags: 0, proto: ETHERTYPE_IPV6 });
    assert_eq!(rest, &TUN_PING[4..]);

    let mut v4 = vec![0x00, 0x01, 0x08, 0x00];
    v4.extend_from_slice(&SYN);
    let (info, rest) = PacketInfo::parse(&v4).unwrap();
    assert_eq!(info, PacketInfo { flags: 1, proto: ETHERTYPE_IPV4 }, "both fields are big-endian");
    assert_eq!(rest, &SYN[..]);

    let (info, rest) = PacketInfo::parse(&[0xab, 0xcd, 0x12, 0x34]).unwrap();
    assert_eq!(info, PacketInfo { flags: 0xabcd, proto: 0x1234 });
    assert!(rest.is_empty());
}

#[test]
fn packet_info_needs_four_bytes() {
    assert_eq!(PacketInfo::parse(&[0, 0, 8]), Err(PacketError::Truncated { needed: 4, got: 3 }));
    assert_eq!(PacketInfo::parse(&[]), Err(PacketError::Truncated { needed: 4, got: 0 }));
}

// ---------------------------------------------------------------- IPv4

#[test]
fn ipv4_parse_every_field() {
    let h = Ipv4Header::parse(&WIKI_IPV4).expect("valid header");
    assert_eq!(h.dscp_ecn, 0);
    assert_eq!(h.total_len, 0x73);
    assert_eq!(h.identification, 0);
    assert!(h.dont_fragment);
    assert!(!h.more_fragments);
    assert_eq!(h.fragment_offset, 0);
    assert_eq!(h.ttl, 64);
    assert_eq!(h.protocol, 0x11);
    assert_eq!(h.protocol(), 0x11);
    assert_eq!(h.header_checksum, 0xb861);
    assert_eq!(h.source, [192, 168, 0, 1]);
    assert_eq!(h.destination, [192, 168, 0, 199]);
    assert_eq!(h.source_addr(), Ipv4Addr::new(192, 168, 0, 1));
    assert_eq!(h.destination_addr(), Ipv4Addr::new(192, 168, 0, 199));
    assert!(h.options.is_empty());
    assert_eq!(h.header_len(), 20);
    assert_eq!(h.payload_len(), 0x73 - 20);
}

#[test]
fn ipv4_parse_reads_only_the_header() {
    // total_len says 115 bytes but only the header is here: that is fine,
    // parsing looks at the header alone (like etherparse's Ipv4HeaderSlice).
    assert!(Ipv4Header::parse(&WIKI_IPV4).is_ok());
    let h = Ipv4Header::parse(&SYN).unwrap();
    assert_eq!(h.protocol, PROTO_TCP);
    assert_eq!(h.total_len as usize, SYN.len());
    assert_eq!(h.identification, 0x1c46);
}

#[test]
fn ipv4_fragment_fields() {
    let mut b = WIKI_IPV4;
    b[6] = 0x20; // MF, offset high bits 0
    b[7] = 0x05;
    let h = Ipv4Header::parse(&b).unwrap();
    assert!(!h.dont_fragment);
    assert!(h.more_fragments);
    assert_eq!(h.fragment_offset, 5);

    b[6] = 0x5f; // DF + 13-bit offset 0x1fff
    b[7] = 0xff;
    let h = Ipv4Header::parse(&b).unwrap();
    assert!(h.dont_fragment);
    assert!(!h.more_fragments);
    assert_eq!(h.fragment_offset, 0x1fff);
    b[1] = 0xb8;
    assert_eq!(Ipv4Header::parse(&b).unwrap().dscp_ecn, 0xb8);
}

#[test]
fn ipv4_options() {
    let mut b = WIKI_IPV4.to_vec();
    b[0] = 0x46; // IHL 6: 4 bytes of options
    b.extend_from_slice(&[0x94, 0x04, 0x00, 0x00]); // router alert
    let h = Ipv4Header::parse(&b).unwrap();
    assert_eq!(h.options, vec![0x94, 0x04, 0x00, 0x00]);
    assert_eq!(h.header_len(), 24);
    assert_eq!(h.payload_len(), 0x73 - 24);
    assert_eq!(h.destination, [192, 168, 0, 199], "fixed fields are unaffected by options");
}

#[test]
fn ipv4_errors_in_order() {
    // 1. fewer than 20 bytes
    assert_eq!(Ipv4Header::parse(&WIKI_IPV4[..19]), Err(PacketError::Truncated { needed: 20, got: 19 }));
    assert_eq!(Ipv4Header::parse(&[]), Err(PacketError::Truncated { needed: 20, got: 0 }));
    // 2. version
    let mut b = WIKI_IPV4;
    b[0] = 0x65;
    assert_eq!(Ipv4Header::parse(&b), Err(PacketError::NotIpv4 { version: 6 }));
    // 3. IHL below 5
    b[0] = 0x44;
    assert_eq!(Ipv4Header::parse(&b), Err(PacketError::BadHeaderLength { value: 4 }));
    // 4. IHL says there are options, but the bytes are missing
    b[0] = 0x46;
    assert_eq!(Ipv4Header::parse(&b), Err(PacketError::Truncated { needed: 24, got: 20 }));
    // 5. total length shorter than the header
    let mut b = WIKI_IPV4;
    b[2] = 0;
    b[3] = 19;
    assert_eq!(Ipv4Header::parse(&b), Err(PacketError::BadTotalLength { total_len: 19, header_len: 20 }));
    b[3] = 20;
    assert!(Ipv4Header::parse(&b).is_ok(), "a header with no payload is fine");
    // the checksum is not verified while parsing
    let mut b = WIKI_IPV4;
    b[10] = 0;
    assert!(Ipv4Header::parse(&b).is_ok());
}

#[test]
fn ipv4_write_round_trips() {
    let h = Ipv4Header::parse(&WIKI_IPV4).unwrap();
    assert_eq!(h.calc_header_checksum(), 0xb861);
    assert_eq!(bytes_of_ip(&h), WIKI_IPV4.to_vec());

    let mut h = Ipv4Header::parse(&SYN).unwrap();
    assert_eq!(bytes_of_ip(&h), SYN[..20].to_vec());
    h.header_checksum = 0xdead;
    assert_eq!(bytes_of_ip(&h), SYN[..20].to_vec(), "write ignores the stored checksum and computes a fresh one");
    assert_eq!(h.header_checksum, 0xdead, "write does not modify the header");

    let mut with_opts = WIKI_IPV4.to_vec();
    with_opts[0] = 0x46;
    with_opts.extend_from_slice(&[0x94, 0x04, 0x00, 0x00]);
    let h = Ipv4Header::parse(&with_opts).unwrap();
    let out = bytes_of_ip(&h);
    assert_eq!(out.len(), 24);
    assert_eq!(&out[20..], &[0x94, 0x04, 0x00, 0x00]);
    assert_eq!(out[0], 0x46);
    assert_eq!(common::checksum(&out), 0, "checksum covers the options");
    assert_eq!(Ipv4Header::parse(&out).unwrap().options, h.options);
}

#[test]
fn ipv4_write_every_field() {
    let h = Ipv4Header {
        dscp_ecn: 0xb8,
        total_len: 1500,
        identification: 0xbeef,
        dont_fragment: false,
        more_fragments: true,
        fragment_offset: 0x0abc,
        ttl: 3,
        protocol: 17,
        header_checksum: 0,
        source: [10, 0, 0, 1],
        destination: [10, 0, 0, 2],
        options: Vec::new(),
    };
    let out = bytes_of_ip(&h);
    assert_eq!(&out[..10], &[0x45, 0xb8, 0x05, 0xdc, 0xbe, 0xef, 0x2a, 0xbc, 3, 17]);
    assert_eq!(&out[12..], &[10, 0, 0, 1, 10, 0, 0, 2]);
    assert_eq!(common::checksum(&out), 0);
    let back = Ipv4Header::parse(&out).unwrap();
    assert_eq!(Ipv4Header { header_checksum: 0, ..back.clone() }, h);
    assert_eq!(back.header_checksum, h.calc_header_checksum());
}

#[test]
fn ipv4_write_refuses_bad_options() {
    let mut h = Ipv4Header::parse(&WIKI_IPV4).unwrap();
    h.options = vec![1, 2, 3];
    let err = h.write(&mut Vec::new()).expect_err("3 bytes of options cannot be expressed in IHL");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    h.options = vec![0; 44];
    let err = h.write(&mut Vec::new()).expect_err("more than 40 bytes of options");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    h.options = vec![0; 40];
    assert!(h.write(&mut Vec::new()).is_ok(), "40 bytes is the maximum, and allowed");
}

#[test]
fn ipv4_new_and_payload_len() {
    let mut h = Ipv4Header::new(20, 64, PROTO_TCP, [192, 168, 0, 2], [192, 168, 0, 1]);
    assert_eq!(h.dscp_ecn, 0);
    assert_eq!(h.total_len, 40);
    assert_eq!(h.identification, 0);
    assert!(h.dont_fragment);
    assert!(!h.more_fragments);
    assert_eq!(h.fragment_offset, 0);
    assert_eq!(h.ttl, 64);
    assert_eq!(h.protocol, PROTO_TCP);
    assert_eq!(h.header_checksum, 0);
    assert_eq!(h.source, [192, 168, 0, 2]);
    assert_eq!(h.destination, [192, 168, 0, 1]);
    assert!(h.options.is_empty());
    assert_eq!(h.payload_len(), 20);

    let out = bytes_of_ip(&h);
    assert_eq!(&out[..10], &[0x45, 0x00, 0x00, 0x28, 0x00, 0x00, 0x40, 0x00, 64, 6]);
    assert_eq!(common::checksum(&out), 0);

    assert_eq!(h.set_payload_len(0), Ok(()));
    assert_eq!(h.total_len, 20);
    assert_eq!(h.set_payload_len(65515), Ok(()));
    assert_eq!(h.total_len, 65535);
    assert_eq!(h.set_payload_len(65516), Err(PacketError::TooLarge { len: 65516, max: 65515 }));
    assert_eq!(h.total_len, 65535, "a failed set_payload_len changes nothing");
    h.options = vec![0; 4];
    assert_eq!(h.set_payload_len(65512), Err(PacketError::TooLarge { len: 65512, max: 65511 }));
    assert_eq!(h.set_payload_len(100), Ok(()));
    assert_eq!(h.total_len, 124, "total length counts the options too");
    assert_eq!(h.payload_len(), 100);

    assert_eq!(Ipv4Header::new(65535, 64, 6, [0; 4], [0; 4]).total_len, 65535, "new saturates instead of overflowing");
    h.total_len = 10;
    assert_eq!(h.payload_len(), 0, "payload_len never underflows");
}

// ---------------------------------------------------------------- TCP

#[test]
fn tcp_parse_syn() {
    let h = TcpHeader::parse(&SYN[20..]).expect("valid TCP header");
    assert_eq!(h.source_port, 51234);
    assert_eq!(h.destination_port, 80);
    assert_eq!(h.sequence_number, 0x1a2b3c4d);
    assert_eq!(h.acknowledgment_number, 0);
    assert!(h.syn && !h.ack && !h.fin && !h.rst && !h.psh && !h.urg && !h.ece && !h.cwr && !h.ns);
    assert_eq!(h.window_size, 64240);
    assert_eq!(h.checksum, 0xfcf6);
    assert_eq!(h.urgent_pointer, 0);
    assert_eq!(h.options, vec![0x02, 0x04, 0x05, 0xb4], "the MSS option");
    assert_eq!(h.header_len(), 24);
    // the accessors the book's listings call
    assert_eq!(h.source_port(), 51234);
    assert_eq!(h.destination_port(), 80);
    assert_eq!(h.sequence_number(), 0x1a2b3c4d);
    assert_eq!(h.window_size(), 64240);
    assert!(h.syn() && !h.ack() && !h.fin() && !h.rst());
}

#[test]
fn tcp_parse_data_segment() {
    let ip = Ipv4Header::parse(&DATA).unwrap();
    let h = TcpHeader::parse(&DATA[ip.header_len()..]).unwrap();
    assert_eq!(h.sequence_number, 0x1a2b3c4e);
    assert_eq!(h.acknowledgment_number, 1);
    assert!(h.ack && h.psh && !h.syn && !h.fin);
    assert_eq!(h.window_size, 502);
    assert_eq!(h.header_len(), 20);
    assert!(h.options.is_empty());
    let data = &DATA[ip.header_len() + h.header_len()..];
    assert_eq!(data, b"hello");
}

#[test]
fn tcp_every_flag_bit() {
    let base = SYN[20..40].to_vec();
    let bits: [(u8, u8, &str); 9] = [
        (12, 0x01, "ns"),
        (13, 0x80, "cwr"),
        (13, 0x40, "ece"),
        (13, 0x20, "urg"),
        (13, 0x10, "ack"),
        (13, 0x08, "psh"),
        (13, 0x04, "rst"),
        (13, 0x02, "syn"),
        (13, 0x01, "fin"),
    ];
    for (byte, bit, name) in bits {
        let mut b = base.clone();
        b[12] = 0x50; // data offset 5, no options
        b[13] = 0;
        b[byte as usize] |= bit;
        let h = TcpHeader::parse(&b).unwrap();
        let set = [h.ns, h.cwr, h.ece, h.urg, h.ack, h.psh, h.rst, h.syn, h.fin];
        let names = ["ns", "cwr", "ece", "urg", "ack", "psh", "rst", "syn", "fin"];
        for (flag, n) in set.iter().zip(names) {
            assert_eq!(*flag, n == name, "with only {} set on the wire, {} should be {}", name, n, n == name);
        }
        assert_eq!(bytes_of_tcp(&h), b, "{} survives a round trip", name);
    }
    let mut b = base.clone();
    b[12] = 0x5e; // reserved bits set: ignored when reading
    assert_eq!(TcpHeader::parse(&b).unwrap().header_len(), 20);
}

#[test]
fn tcp_errors() {
    assert_eq!(TcpHeader::parse(&SYN[20..39]), Err(PacketError::Truncated { needed: 20, got: 19 }));
    let mut b = SYN[20..].to_vec();
    b[12] = 0x40;
    assert_eq!(TcpHeader::parse(&b), Err(PacketError::BadHeaderLength { value: 4 }));
    b[12] = 0x70; // 28 bytes claimed, 24 present
    assert_eq!(TcpHeader::parse(&b), Err(PacketError::Truncated { needed: 28, got: 24 }));
    b[12] = 0xf0; // 60 bytes, the maximum
    b.resize(60, 1);
    let h = TcpHeader::parse(&b).unwrap();
    assert_eq!(h.header_len(), 60);
    assert_eq!(h.options.len(), 40);
}

#[test]
fn tcp_checksum_matches_real_packets() {
    for packet in [&SYN[..], &DATA[..]] {
        let ip = Ipv4Header::parse(packet).unwrap();
        let tcph = TcpHeader::parse(&packet[ip.header_len()..]).unwrap();
        let payload = &packet[ip.header_len() + tcph.header_len()..];
        assert_eq!(
            tcph.calc_checksum_ipv4(&ip, payload),
            Ok(tcph.checksum),
            "checksum over pseudo-header, header (checksum field as zero) and payload"
        );
        let mut changed = tcph.clone();
        changed.checksum = 0x1234;
        assert_eq!(changed.calc_checksum_ipv4(&ip, payload), Ok(tcph.checksum), "the stored checksum is ignored");
    }
}

#[test]
fn tcp_checksum_too_large() {
    let ip = Ipv4Header::new(0, 64, PROTO_TCP, [1, 1, 1, 1], [2, 2, 2, 2]);
    let h = TcpHeader::new(1, 2, 3, 4);
    let payload = vec![0u8; 65516];
    assert_eq!(h.calc_checksum_ipv4(&ip, &payload), Err(PacketError::TooLarge { len: 65536, max: 65535 }));
    assert!(h.calc_checksum_ipv4(&ip, &payload[..65515]).is_ok());
}

#[test]
fn tcp_write_round_trips() {
    for packet in [&SYN[..], &DATA[..]] {
        let tcp_bytes = &packet[20..];
        let h = TcpHeader::parse(tcp_bytes).unwrap();
        let out = bytes_of_tcp(&h);
        assert_eq!(out, tcp_bytes[..h.header_len()].to_vec(), "write emits the stored checksum verbatim");
    }
    let mut h = TcpHeader::parse(&SYN[20..]).unwrap();
    h.options = vec![1, 2];
    assert_eq!(h.write(&mut Vec::new()).expect_err("2 bytes of options").kind(), ErrorKind::InvalidData);
}

#[test]
fn tcp_new() {
    let h = TcpHeader::new(80, 51234, 7, 10);
    assert_eq!((h.source_port, h.destination_port, h.sequence_number, h.window_size), (80, 51234, 7, 10));
    assert_eq!(h.acknowledgment_number, 0);
    assert!(!(h.ns || h.cwr || h.ece || h.urg || h.ack || h.psh || h.rst || h.syn || h.fin));
    assert_eq!(h.checksum, 0);
    assert_eq!(h.urgent_pointer, 0);
    assert!(h.options.is_empty());
    assert_eq!(h.header_len(), 20);
    let out = bytes_of_tcp(&h);
    assert_eq!(out, vec![0, 80, 0xc8, 0x22, 0, 0, 0, 7, 0, 0, 0, 0, 0x50, 0, 0, 10, 0, 0, 0, 0]);
}

/// What chapter 3 does with these pieces: build a SYN-ACK from scratch.
#[test]
fn build_a_syn_ack_a_peer_would_accept() {
    let mut tcp = TcpHeader::new(80, 51234, 0, 10);
    tcp.acknowledgment_number = 0x1a2b3c4e;
    tcp.syn = true;
    tcp.ack = true;
    let mut ip = Ipv4Header::new(0, 64, PROTO_TCP, [192, 168, 0, 2], [192, 168, 0, 1]);
    ip.set_payload_len(tcp.header_len()).unwrap();
    tcp.checksum = tcp.calc_checksum_ipv4(&ip, &[]).unwrap();
    let mut datagram = Vec::new();
    ip.write(&mut datagram).unwrap();
    tcp.write(&mut datagram).unwrap();

    let seg = common::decode(&datagram); // checks both checksums and the lengths
    assert_eq!((seg.sport, seg.dport, seg.seq, seg.ack), (80, 51234, 0, 0x1a2b3c4e));
    assert_eq!(seg.flags, common::SYN | common::ACK);
    assert_eq!(seg.window, 10);
    assert!(seg.payload.is_empty());

    // and one with a payload
    let payload = b"hi there";
    let mut tcp = TcpHeader::new(80, 51234, 1, 10);
    tcp.ack = true;
    tcp.psh = true;
    ip.set_payload_len(tcp.header_len() + payload.len()).unwrap();
    tcp.checksum = tcp.calc_checksum_ipv4(&ip, payload).unwrap();
    let mut datagram = Vec::new();
    ip.write(&mut datagram).unwrap();
    tcp.write(&mut datagram).unwrap();
    datagram.extend_from_slice(payload);
    assert_eq!(common::decode(&datagram).payload, payload.to_vec());
}
