# Chapter 6 Outline (working notes, not part of the book)

**Scope:** the second half of the original Chapter 5 plan, moved here when Chapter 5 was split. Give `TcpStream` working `Read` and `Write`.

**Approach:** same as Chapter 5. Jon's streams are the inspiration, but the code is our own design, built on the `Interface` / `ConnectionTable` / `Shared` types from Chapter 5. Link his version at the end only as "compare notes".

**Voice reminders:** first-person plural, explain the *why* before the code, `**Aside** <br>` blocks, RFC quotes in `>` blockquotes, Title Case `###` headings, heavily commented code labelled with the file name, end with "What's next on our agenda?" and then `### References`.

## Rough plan
1. **Opening:** pick up from Chapter 5's empty-shell streams (keep the intro paragraph in `chapter6.md`).
2. **Receiving data in `on_packet`:** drop the `assert!(data.is_empty())`, append in-order payload to a new `incoming: VecDeque<u8>`, advance `recv.nxt`, ACK it.
3. **Handling the client's FIN in `Estab`:** move to `CloseWait` and ACK it. This fixes the retransmitted-FIN loose end from Chapter 5.
4. **Waking readers:** a second `Condvar` on `Shared` (e.g. `readable`). The packet loop notifies it when a connection gains data or sees a FIN. Decide between a small "what changed" return value from `on_packet` and comparing state before and after, which is what Chapter 5 does for `is_established`.
5. **`impl Read for TcpStream`:** block until there's data or EOF. Copy out of both halves of the `VecDeque` (`as_slices`), being careful with `copy_from_slice` lengths (good Aside material: it panics on mismatched lengths). Return `Ok(0)` once the peer has closed and `incoming` is drained.
6. **`impl Write for TcpStream`:** append to `unacked` up to a cap and return how much was accepted. Explain Chapter 4's `unacked` queue at last.
7. **Actually sending:** something has to call `write` to transmit queued bytes. Either nudge it from `TcpStream::write`, or have the packet loop flush on each wakeup. Covers ACKs freeing bytes from `unacked` (the "handle new ACKs" TODO from Chapter 4).
8. **Testing It Out:** an echo server in `main.rs`, `nc 192.168.0.2 8000`, type and see it come back. Get a real capture if possible.
9. **Loose Ends / What's next:** retransmission and timers, closing a stream from our side (`Drop` sending FIN), TIME-WAIT.

## Open questions
- Chapter 4's `Corresponding Code` link points at `fa1665f`, but the `write` it explains is from a later version. Fix the link, or reword it as "compare notes" to match Chapter 5?
- If this chapter also runs long, split at step 7 (sending) and move it into Chapter 7.
