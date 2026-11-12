# Code companion

The book deliberately leaves out complete code, so that you write it yourself.
This directory is for afterwards: a crate (package `tcp`) that follows the
chapters and ends where chapter 5 ends. Write your own version first, then
compare notes.

It uses only the standard library. The book uses two crates; here they are
chapters of their own:

| Book uses | Here | Chapter |
| --- | --- | --- |
| `tun-tap` | [`src/tun.rs`](src/tun.rs): `/dev/net/tun` and one `ioctl(TUNSETIFF)` | 1 |
| `etherparse` | [`src/packet/`](src/packet/): IPv4 and TCP headers, checksums | 2 |

The listings translate one for one. Where the book passes
`nic: &mut tun_tap::Iface`, this crate passes `nic: &mut dyn Nic`, a two-method
trait (`recv`, `send`) that the TUN device implements and the tests fake.
Headers are plain structs: `tcph.syn()` and `iph.source_addr()` work as in the
book, and so does `tcp.syn = true`.

## Where everything is

| Chapter | File | Task |
| --- | --- | --- |
| 1. The kernel-user space divide | `src/tun.rs` | `0001-tun-device` |
| 2. Parsing the bits | `src/packet/info.rs`, `ipv4.rs`, `tcp_header.rs` | `0002-parsing-the-bits` |
| 3. The connection quad | `src/tcp/accept.rs` | `0003-connection-quad` |
| 4. The final handshake | `src/tcp/seq.rs`, `write.rs`, `on_packet.rs` | `0004-final-handshake` |
| 5. The listening socket | `src/interface.rs` | `0005-listening-socket` |

The shared types (`Quad`, `State`, `Connection`, the sequence spaces, the
header structs) and every public signature are already in place. The function
bodies arrive one task at a time; a task is done when `ci/done/<task-id>`
exists on `main`. Until then that chapter's functions are `todo!()`.

Some code sits in a different chapter here than in the book, so that no file
belongs to two tasks:

- The book's chapter 4 rewrites `accept` to call `write`. Here `accept` keeps
  building its own SYN-ACK, as in the chapter 3 listing.
- The block that hangs up straight after the handshake is gone, as in chapter 5.
  It survives as `Connection::close`.

The book keeps `tcp::Connection` private. Here it is `pub`, so each chapter can
be tested on its own.

## Run it

You need Linux. The binary is chapter 5's `main.rs`: it listens on ports 8000
and 9000. This is chapter 1's run script, run from `code/`:

```sh
cargo b --release
sudo setcap cap_net_admin=eip ./target/release/tcp
./target/release/tcp &
pid=$!
sudo ip addr add 192.168.0.1/24 dev tun0
sudo ip link set up dev tun0
trap "kill $pid" INT TERM
wait $pid
```

Then `nc 192.168.0.2 8000` from another terminal.

## Test it

Nothing here needs a TUN device or root. Each chapter has an acceptance test
in `acceptance/` that works on packet bytes and state machines, with an
in-memory NIC:

```sh
sh ci/check.sh                                # what CI runs
cargo test --offline --test t0004_final_handshake   # one chapter
```

`acceptance/common/` is the judge's own packet encoder and decoder, written
separately from `src/packet/`. That way the later chapters' tests check the
bytes on the wire, and do not rely on chapter 2 to read back what it wrote.

## How it gets written

Each chapter is a signed task on [AgentFund](https://agentfund.dev): people
lend their AI coding agent, it implements one chapter in a sandbox, and CI
decides whether it counts. [CONTRIBUTING.md](../CONTRIBUTING.md) has the rules
for agents and humans alike, and the [README](../README.md#code-companion)
explains how to back it.
