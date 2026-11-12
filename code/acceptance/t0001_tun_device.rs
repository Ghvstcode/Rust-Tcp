//! Acceptance test for task 0001-tun-device (chapter 1). Outside every task's write scope.
//!
//! Opening a real TUN device needs Linux and cap_net_admin, which neither CI
//! nor the task sandbox has. So this checks everything that can be checked
//! without one: the exact bytes of the `struct ifreq` handed to
//! `ioctl(TUNSETIFF)`, reading the name back, and that bad names are refused
//! before any device is touched.

use std::io::ErrorKind;

use tcp::tun::{ifreq, ifreq_name, Iface, Mode, IFF_NO_PI, IFF_TAP, IFF_TUN, IFNAMSIZ, IFREQ_LEN};
use tcp::Nic;

fn flags_of(ifr: &[u8; IFREQ_LEN]) -> i16 {
    i16::from_ne_bytes([ifr[IFNAMSIZ], ifr[IFNAMSIZ + 1]])
}

#[test]
fn constants_match_linux() {
    assert_eq!(IFF_TUN, 0x0001);
    assert_eq!(IFF_TAP, 0x0002);
    assert_eq!(IFF_NO_PI, 0x1000);
    assert_eq!(IFNAMSIZ, 16);
    assert_eq!(IFREQ_LEN, 40);
    assert_eq!(tcp::tun::TUNSETIFF, 0x4004_54ca);
}

#[test]
fn tun0_with_packet_info() {
    let ifr = ifreq("tun0", Mode::Tun, true).expect("tun0 is a valid name");
    assert_eq!(&ifr[..4], b"tun0");
    assert!(ifr[4..IFNAMSIZ].iter().all(|&b| b == 0), "the rest of the name field is NUL: {:?}", ifr);
    assert_eq!(flags_of(&ifr), IFF_TUN, "packet info on means no IFF_NO_PI");
    assert!(ifr[IFNAMSIZ + 2..].iter().all(|&b| b == 0), "everything after the flags is zero: {:?}", ifr);
}

#[test]
fn tun0_without_packet_info() {
    let ifr = ifreq("tun0", Mode::Tun, false).unwrap();
    assert_eq!(&ifr[..4], b"tun0");
    assert_eq!(flags_of(&ifr), IFF_TUN | IFF_NO_PI);
    assert_eq!(&ifr[IFNAMSIZ..IFNAMSIZ + 2], &(IFF_TUN | IFF_NO_PI).to_ne_bytes(), "flags are in native byte order");
}

#[test]
fn tap_mode() {
    assert_eq!(flags_of(&ifreq("tap7", Mode::Tap, true).unwrap()), IFF_TAP);
    assert_eq!(flags_of(&ifreq("tap7", Mode::Tap, false).unwrap()), IFF_TAP | IFF_NO_PI);
}

#[test]
fn names_up_to_fifteen_bytes() {
    let name = "abcdefghijklmno"; // 15 bytes: the 16th is the terminating NUL
    let ifr = ifreq(name, Mode::Tun, false).expect("15 bytes fit");
    assert_eq!(&ifr[..15], name.as_bytes());
    assert_eq!(ifr[15], 0);

    let err = ifreq("abcdefghijklmnop", Mode::Tun, false).expect_err("16 bytes do not fit");
    assert_eq!(err.kind(), ErrorKind::InvalidInput);

    let err = ifreq("ééééééééé", Mode::Tun, false).expect_err("the limit is in bytes, not characters (18 bytes)");
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
}

#[test]
fn kernel_templates_and_empty_names_are_allowed() {
    let ifr = ifreq("tun%d", Mode::Tun, false).unwrap();
    assert_eq!(&ifr[..5], b"tun%d");
    let ifr = ifreq("", Mode::Tun, false).expect("an empty name lets the kernel choose one");
    assert!(ifr[..IFNAMSIZ].iter().all(|&b| b == 0));
}

#[test]
fn nul_bytes_are_refused() {
    let err = ifreq("tu\0n", Mode::Tun, false).expect_err("a NUL would cut the name short");
    assert_eq!(err.kind(), ErrorKind::InvalidInput);
}

#[test]
fn name_round_trip() {
    for name in ["tun0", "t", "abcdefghijklmno", ""] {
        let ifr = ifreq(name, Mode::Tun, true).unwrap();
        assert_eq!(ifreq_name(&ifr).unwrap(), name);
    }
}

#[test]
fn name_written_by_the_kernel() {
    // The kernel replaces "tun%d" with the name it picked, in place.
    let mut ifr = ifreq("tun%d", Mode::Tun, false).unwrap();
    ifr[..IFNAMSIZ].copy_from_slice(b"tun3\0\0\0\0\0\0\0\0\0\0\0\0");
    assert_eq!(ifreq_name(&ifr).unwrap(), "tun3");

    // Anything after the first NUL is ignored.
    ifr[..IFNAMSIZ].copy_from_slice(b"tun4\0garbage\0\0\0\0");
    assert_eq!(ifreq_name(&ifr).unwrap(), "tun4");
}

#[test]
fn unreadable_names_are_errors() {
    let mut ifr = [0u8; IFREQ_LEN];
    ifr[..IFNAMSIZ].copy_from_slice(&[b'x'; IFNAMSIZ]);
    let err = ifreq_name(&ifr).expect_err("no NUL inside the 16-byte name field");
    assert_eq!(err.kind(), ErrorKind::InvalidData);

    let mut ifr = [0u8; IFREQ_LEN];
    ifr[..3].copy_from_slice(&[0xff, 0xfe, 0xfd]);
    let err = ifreq_name(&ifr).expect_err("not UTF-8");
    assert_eq!(err.kind(), ErrorKind::InvalidData);
}

#[test]
fn bad_names_fail_before_touching_any_device() {
    for open in [Iface::new, Iface::without_packet_info] {
        match open("much-too-long-for-linux", Mode::Tun) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::InvalidInput),
            Ok(_) => panic!("a 23-byte name must be refused"),
        }
        match open("bad\0name", Mode::Tun) {
            Err(e) => assert_eq!(e.kind(), ErrorKind::InvalidInput),
            Ok(_) => panic!("a name with a NUL must be refused"),
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn other_systems_say_unsupported() {
    match Iface::without_packet_info("tun0", Mode::Tun) {
        Err(e) => assert_eq!(e.kind(), ErrorKind::Unsupported),
        Ok(_) => panic!("TUN devices are Linux-only here"),
    }
}

#[test]
fn iface_is_a_nic_the_packet_loop_can_own() {
    fn assert_nic<T: Nic + Send + 'static>() {}
    assert_nic::<Iface>();
}
