# Chapter 5 Outline (working notes, not part of the book)

**Working title:** *Opening For Business* (alternatives: *A Socket Of Our Own*, *Listening In*)

**Goal of the chapter:** turn the "answer every SYN on every port" loop in `main.rs` into a small library with an API that feels like `std::net`: an `Interface` you create once, `bind(port)` that gives you a `TcpListener`, `accept()` that hands back a `TcpStream`, and `Read`/`Write` on that stream. The packet loop moves onto its own thread, so we now have to share state across threads.

**Corresponding code (Jon's repo):**
- [`759baab` – Add nice net-like interface](https://github.com/jonhoo/rust-tcp/commit/759baab5e245ba859ff52d3ce85881961497ea4b): `Interface`, `bind`, `TcpListener`, `TcpStream`, the packet loop thread
- [`fa1665f` – Make impl Read for TcpStream work for FIN](https://github.com/jonhoo/rust-tcp/commit/fa1665f4141e205cde056d5eaa872933169e2fac): blocking `read` with a `Condvar`, `Available` bitflags, `read` returning 0 on FIN
- Pull the read-buffer bug fix forward from [`9d59d7f`](https://github.com/jonhoo/rust-tcp/commit/9d59d7f8023faae3fd8d2839ce9ff6c1cac4bd5d) (see section 6)

**Voice reminders (match chapters 1–4):** first-person plural ("we", "our stack"), conversational, open with where we left off, explain the *why* before the code, `**Aside** <br>` blocks for side topics, RFC quotes in `>` blockquotes, Title Case `###` headings, heavily commented code blocks labelled with the file name (`// lib.rs`), end with "What's next on our agenda?" + a playful hand-off, then `### References`.

---

## 0. Opening (keep the existing paragraph)
- Keep the one paragraph already in `chapter5.md` as the intro. It already sets up the chapter's goal.
- Add a short bridge from Chapter 4's ending: "our stack is a little too eager" → today we give it manners.
- Introduce the target API up front with the small `main.rs` we want to write at the end of the chapter, so readers see where we're going (`Interface::new()`, `bind(8000)`, `accept()` in a loop, `read`).

## 1. Taking A Page From The Standard Library
- What `std::net::TcpListener` / `TcpStream` look like to a user (`bind`, `incoming`/`accept`, `Read`/`Write`, `Drop` closes).
- Our version needs one extra thing the std one hides: the `Interface` (the kernel owns that part normally; we are the kernel now, a callback to the Chapter 1 kernel/user-space framing).
- Turning the crate into a library + binary: `[lib]` / `[[bin]]` in `Cargo.toml`, move code into `lib.rs`, `main.rs` becomes a user of our library.
  - **Decision needed:** Jon calls the crate `trust`. The book's `run.sh` uses `PKG_NAME=tcp`. Keep `tcp` for continuity, or rename and update `run.sh`?
- `Quad` moves into `lib.rs` (it's shared by the manager and the streams).

## 2. The Interface And Its Packet Loop
- `Interface::new()` creates the TUN device and **spawns a thread** running `packet_loop(nic, ih)`.
- The body of `packet_loop` is basically the old `main` loop from Chapters 2–4. Point readers back to it instead of re-explaining the parsing.
- `Interface` holds `Option<InterfaceHandle>` + `Option<JoinHandle<io::Result<()>>>`. Explain why they're `Option`s (so `Drop` can `take()` them).
- `impl Drop for Interface`: set `terminate`, drop our handle, join the thread.
  - Be honest that `terminate` isn't actually checked yet (the loop is blocked in `recv`): a TODO for the timers chapter.
- **Aside:** `without_packet_info` vs `new` on `tun_tap::Iface`. This removes the 4 bytes of flags/proto we parsed by hand in Chapter 2. Tie back to the Tun/TAP doc link in Chapter 2 so readers aren't confused when the `buf[4..]` offset disappears.

## 3. Sharing State Across Threads
- The problem: the packet loop thread and the user's threads (calling `accept`/`read`/`write`) all need the connection table.
- `ConnectionManager { terminate, connections: HashMap<Quad, Connection>, pending: HashMap<u16, VecDeque<Quad>> }`.
  - `connections` is the Chapter 3 map, moved here.
  - `pending` is new: per-port queue of established-but-not-yet-accepted connections (this *is* the listen backlog).
- Wrap it: `Arc<Foobar { manager: Mutex<ConnectionManager>, pending_var: Condvar }>` = `InterfaceHandle`.
  - Have some fun with Jon's `Foobar` name; suggest a better one (e.g. `InterfaceInner`) and use that in the book.
- Short explanation of `Arc` (shared ownership across threads), `Mutex` (one at a time), `Condvar` (sleep until someone says "something changed"). Keep it practical, not a Rust concurrency tutorial: link to the Rust book chapter on fearless concurrency.
- **Aside:** why one big lock is fine for us (simplicity over throughput) and what a real stack would do.

## 4. Binding To A Port
- `Interface::bind(port)`: lock the manager, insert an empty `VecDeque` into `pending` for that port, return `TcpListener { port, h }`.
- If the port is already there → `io::ErrorKind::AddrInUse`. Compare with the error you get from `std::net::TcpListener::bind` on a busy port.
- Update `packet_loop`'s `Entry::Vacant` arm: only call `Connection::accept` **if** `pending` has an entry for `tcph.destination_port()`. Otherwise ignore the SYN.
  - This is the payoff for Chapter 4's "we still accept a SYN on every port".
  - Mention what RFC 793 says should happen to a SYN for a closed port (send RST) and note we're silently dropping for now. Quote the RFC's "If the state is CLOSED..." paragraph in section 3.9.

## 5. Accepting Connections
- In `packet_loop`, after `Connection::accept` succeeds: insert into `connections`, `push_back` the quad onto `pending[port]`, **drop the lock**, then `pending_var.notify_all()`.
  - Explain why we drop the guard before notifying (the woken thread would immediately block on the mutex we're still holding).
- `TcpListener::accept()`: lock, loop { pop_front from pending[port] → return `TcpStream { quad, h }`; else `cm = pending_var.wait(cm)` }.
  - Walk through the `Condvar::wait` dance: it releases the lock while sleeping and re-acquires it on wake-up. Spurious wake-ups are why it's a `loop`.
- **Discussion point:** Jon queues the connection right after sending SYN-ACK (in `SynRcvd`), not once it's `Estab`. A real stack only hands you established connections. Flag it in an Aside and either fix it in the book's code or list it under loose ends.
- `impl Drop for TcpListener`: remove the port from `pending`. The leftover queued connections are `unimplemented!()` in Jon's code. Say so and add to loose ends.

## 6. Reading From A Stream
- New fields on `Connection`: `incoming: VecDeque<u8>` (and `unacked`, which readers already met in Chapter 4's `write`). Callback: "remember that unacked queue we promised to explain?" Explain `incoming` here and leave `unacked` for the data chapter.
- `impl Read for TcpStream` (final `fa1665f` shape):
  1. Lock, look up our quad. Missing → `ConnectionAborted`.
  2. If `is_rcv_closed()` and `incoming` is empty → `Ok(0)` (EOF, the same contract as std).
  3. If there's data → copy out of the two halves of the ring buffer (`as_slices`, same idea as `write` in Ch. 4), `drain(..nread)`, return.
  4. Otherwise `cm = rcv_var.wait(cm)` and loop.
- **Use the fixed copy code** (`buf[..hread].copy_from_slice(...)` / `buf[hread..hread + tread]...`). Jon's `fa1665f` version calls `buf.copy_from_slice(...)`, which panics unless the lengths match exactly. Good material for an Aside on the `copy_from_slice` length rule.
- Waking readers: add `rcv_var: Condvar`, make `on_packet` return an `Available` bitflag (`READ`/`WRITE`), and have `packet_loop` notify `rcv_var` when `READ` is set. `Connection::availability()` and `is_rcv_closed()` (only `TimeWait` for now, TODO for the other post-FIN states).
  - New dependency: `bitflags`. Brief intro.
  - **Heads-up:** the current `bitflags` (2.x) syntax differs from Jon's 1.x version. Show the 2.x form, or pin 1.x in `Cargo.toml`. Decide when drafting.

## 7. Writing To A Stream (API only)
- `impl Write for TcpStream`: append to `unacked` up to `SENDQUEUE_SIZE` (1024); return `WouldBlock` when full; `flush` succeeds only when `unacked` is empty.
- Be upfront that **nothing actually transmits this data yet**. The `write` from Chapter 4 will read from `unacked`, but nothing triggers sending. Tease the next chapter.
- `TcpStream::shutdown` and `impl Drop for TcpStream` are TODOs (should send FIN). List under loose ends.

## 8. Testing It Out
- New `main.rs`: `Interface::new()`, `bind(8000)`, spawn a thread that `accept`s in a loop, `read`s into a 1-byte buffer and asserts it got 0 (EOF).
- Run `run.sh`, `tshark -i tun0`, then:
  - `nc 192.168.0.2 8000`: handshake, our FIN, their FIN, `read` returns 0, "no more data!" in our logs.
  - `nc 192.168.0.2 9000` (a port we didn't bind): SYN with no reply (and the client retrying). Shows the port filtering working.
- **Ideally capture this from a real run** (Chapter 4's capture is illustrative; this would be a good place to show the real thing).
- Show the `eprintln!` lines from `packet_loop` ("got packet for unknown quad", "listening, so accepting").

## 9. Loose Ends
- `Interface::drop` sets `terminate`, but the loop never checks it (blocked in `recv`, needs a timeout → timers chapter).
- `TcpListener::drop` with queued connections → `unimplemented!()`.
- `TcpStream::drop` / `shutdown` don't send FIN yet.
- `Write` buffers data but never transmits it.
- `is_rcv_closed` only recognises `TimeWait`.
- SYNs to unbound ports are dropped instead of RST'd.
- Connections are handed to `accept` before the handshake completes (if not fixed in section 5).

## 10. What's Next On Our Agenda?
- Recap: library crate, packet loop on its own thread, `bind`/`accept`, blocking reads with `Condvar`, EOF on FIN.
- Hand-off: our stream can *read*, and `write` happily queues bytes, but nobody ever sends them. Next chapter: actually moving data (sending from `unacked`, handling ACKs that free it, `Available::WRITE`, and a clean shutdown when the stream is dropped).

## References
- Corresponding Code: `759baab`, `fa1665f` (+ the read fix from `9d59d7f`)
- `std::net::TcpListener` / `TcpStream` docs
- `std::sync::Condvar` docs
- RFC 793 §3.9 (Event Processing: OPEN / SEGMENT ARRIVES in CLOSED / LISTEN)

---

## Things to resolve before drafting
1. **Chapter 4's reference link.** Chapter 4 explains the *final* version of `write` (`closed_at`, `timers.send_times`, slicing `unacked`), which only appears from `72d2443` on. But it links to `fa1665f`, which is actually the end of *this* chapter's material. Options: (a) point Chapter 4 at `ad5f0bc` and say its `write` is the polished version we'll grow into, or (b) leave as is and add a note in Chapter 5 that readers already have the fuller `write`. (a) is cleaner.
2. **Crate name:** `tcp` (book) vs `trust` (Jon).
3. **`bitflags` version:** 1.x (matches Jon) vs 2.x (current).
4. **Accept-before-established:** fix in the book's code, or just flag it.
5. **Chapter length:** this is meatier than Chapter 4. If it runs long, split at section 6: Chapter 5 = Interface/bind/accept, Chapter 6 = Read/Write on the stream.
