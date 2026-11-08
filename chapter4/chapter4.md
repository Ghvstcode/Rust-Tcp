## The Final Handshake
Moving on to the `on_packet` method, we step into a crucial juncture in the TCP three-way handshake. After sending a SYN-ACK, the server engages this method as it awaits the client's ACK. This acknowledgment propels the connection from the "SYN-RECEIVED" state into the "ESTABLISHED" one, cementing the handshake and formally opening the communication channel. The `on_packet` method does the heavy lifting of finalizing the three-way handshake
It checks for the ACK response from the client. If the ACK is correctly received, the method will shift the connection's status to "ESTABLISHED," signifying that the TCP connection is complete and data transfer can begin.

Before we continue with our implementation, we should add a helper method "write" that will be used by both the `on_accept` and `on_packet` connection methods. This method will abstract away the complexity of constructing and sending TCP segments by consolidating common tasks performed by the `on_accept` and `on_packet` connection methods into one place.

Within the `write` method, the sequence and acknowledgment numbers are first set to the appropriate values for the outgoing packet.
* The acknowledgment number is configured to `self.recv.nxt`, reflecting the receiver's next expected byte. The acknowledgment number is crucial for the proper sequencing of the data, as it indicates the successful receipt of all data up to this point and what the connection is prepared to receive next. This accounting starts from the initial receive sequence number (IRS), combined with the number of bytes received so far.
* The sequence number for the packet to be sent is passed into the function and updated to the `self.tcp.sequence_number` field, ensuring that the TCP segment carries the correct sequence information. The `write` method also determines the maximum amount of data it can send in a single segment, which as discussed earlier is referred to as the limit within the function.
* Subsequent to the initial sequence setting, the method proceeds to prepare the outgoing TCP segment. The segment's data payload is derived from the `self.unacked` queue, which contains the data bytes that have been sent but not yet acknowledged. We are yet to discuss the unacked queue - so for now I will skip the explanation of this and revisit it later on when we are talking about Data transmission.
* The write method, after assembling the payload data then proceeds to write the IP header into `buf`, updating the payload length field to include the actual size of the TCP payload. It deliberately leaves space for the TCP header, which will be filled in after the payload is written because the TCP checksum calculation requires the complete segment, including both headers and payload.
* After writing the payload, the TCP checksum is calculated over the entire segment, including the pseudo-header, TCP header, and payload, for error-checking. Once the checksum is computed, the TCP header is written into the reserved space in `buf`. The transmission control flags, like `SYN` and `FIN`, also influence the sequence space. If either flag is set, `write` method increments the sequence number since these flags consume a sequence number in the handshake or termination process.
* As it finishes constructing the packet, the method updates various state variables such as `next_seq`, which tracks the next sequence number expected by the sender after accounting for any transmitted data and control flags. It then logs the current time against the sequence number in `self.timers.send_times` (will visit later) for later use in estimating the round-trip time (RTT) and managing retransmissions.

```Rust
// tcp.rust
// Define the 'write' method for the 'Connection' struct.
fn write(&mut self, nic: &mut tun_tap::Iface, seq: u32, mut limit: usize) -> io::Result<usize> {
    // Allocate a local buffer for creating the TCP segment.
    // The size is set to 1500 bytes, which is the standard maximum transmission unit (MTU) for Ethernet.
    let mut buf = [0u8; 1500];

    // The sequence number for the outgoing packet is determined by the caller.
    // This is the number that keeps track of the position of the first byte in the current message 
    // within the entire sequence of sent bytes.
    self.tcp.sequence_number = seq;

    // The acknowledgment number is what our side expects to receive next from the other side. 
    // It is calculated as the sum of the initial received sequence number (irs) and the number of bytes received so far.
    self.tcp.acknowledgment_number = self.recv.nxt; 

    // Ensure we don't send more data than we are allowed, which includes respecting the send window limit, 
    // which is the amount of data we can send before needing an acknowledgment, and accounting for any SYN/FIN flags 
    // which also consume sequence space despite not being part of the window.

    // Print out information to help with debugging the connection status.
    // The difference between 'self.recv.nxt' and 'self.recv.irs' gives the number of bytes we have acknowledged.
    // 'seq' shows the starting sequence number for this segment.
    // 'limit' is the maximum amount of data we can send in this packet.
    println!(
        "write(ack: {}, seq: {}, limit: {}) syn {:?} fin {:?}",
        self.recv.nxt - self.recv.irs,
        seq,
        limit,
        self.tcp.syn,
        self.tcp.fin,
    );

    // Determine the starting point for data to write in the current TCP stream.
    // 'wrapping_sub' is used to safely calculate differences of sequence numbers considering they might wrap around.
    // Sequence numbers in TCP are 32-bit unsigned numbers and wrap back to 0 after 2^32 - 1.
    let mut offset = seq.wrapping_sub(self.send.una) as usize;

    // If the connection is closing, ensure no more data is sent after FIN.
    if let Some(closed_at) = self.closed_at {
        if seq == closed_at.wrapping_add(1) {
            // After sending a FIN, reset the offset and limit to prevent further data transmission.
            offset = 0;
            limit = 0;
        }
    }

    // Print the calculated offset for debugging.
    println!("using offset {} base {} in {:?}", offset, self.send.una, self.unacked.as_slices());

    // Split the unsent data into two slices - 'h' as the head and 't' as the tail of the unsent data.
    // The split is based on the current 'offset' from where we need to start sending data.
    let (mut h, mut t) = self.unacked.as_slices();
    if h.len() > offset {
        // If 'h' is longer than 'offset', we truncate 'h' to start from 'offset'.
        h = &h[offset..];
    } else {
        // If 'h' is shorter, we skip 'h' completely and adjust 't' accordingly.
        let skipped = h.len();
        h = &[]; // 'h' is now empty, as it's been fully sent.
        t = &t[(offset - skipped)..]; // 't' is adjusted to remove the already sent portion.
    }

    // Calculate how much data can be sent by choosing the smaller of 'limit' or available data.
    let max_data = std::cmp::min(limit, h.len() + t.len());
    let size = std::cmp::min(
        // Limit the size to prevent buffer overflow.
        buf.len(),
        // Include only as much data as the MTU allows after accounting for IP and TCP header sizes.
        self.tcp.header_len() as usize + self.ip.header_len() as usize + max_data,
    );

    // Set the IP packet's payload length to the TCP segment size.
    self.ip.set_payload_len(size - self.ip.header_len() as usize);

    // Start writing the packet with the IP header first.
    // 'Write' is a trait in Rust that allows writing bytes to a buffer. Here we use it to write the IP header.
    use std::io::Write;
    let buf_len = buf.len();
    let mut unwritten = &mut buf[..]; // Borrow the entire buffer as mutable to start writing the IP header.

    self.ip.write(&mut unwritten); // Writing the IP header into the buffer.
    // Calculate the point in the buffer where the IP header ends based on how much was written.
    let ip_header_ends_at = buf_len - unwritten.len();

    // Leave space for the TCP header by skipping over the region where it will be written.
    // This is done to come back later and fill it in once we know the payload and can compute the checksum.
    unwritten = &mut unwritten[self.tcp.header_len() as usize..];
    let tcp_header_ends_at = buf_len - unwritten.len(); // Identify where the TCP header will be written.

    // Now write the TCP payload to the buffer by first writing the head slice 'h' and then the tail 't'.
    let payload_bytes = {
        let mut written = 0; // This will track how many bytes we've successfully written.
        let mut limit = max_data; // The adjusted payload size after accounting for the TCP header space.

        // Write the first part of the payload ('h') until we hit the limit or run out of 'h'.
        let p1l = std::cmp::min(limit, h.len()); // 'p1l' is how much we can write from 'h'.
        written += unwritten.write(&h[..p1l])?; // Write and update 'written' with bytes written.
        limit -= written; // Reduce the limit by what's been written so far.

        // Continue with 't', writing as much as we can after 'h' has been accounted for.
        let p2l = std::cmp::min(limit, t.len()); // 'p2l' is how much we can write from 't'.
        written += unwritten.write(&t[..p2l])?; // Write 't' to the buffer and update the count.
        written // Return total written payload size.
    };

    // Calculate the end of the payload to know where to stop writing to the buffer.
    let payload_ends_at = buf_len - unwritten.len();

    // Compute the TCP checksum which ensures data integrity over the network.
    // The checksum needs the IP header and the payload to be calculated correctly.
    self.tcp.checksum = self
        .tcp
        .calc_checksum_ipv4(&self.ip, &buf[tcp_header_ends_at..payload_ends_at])
        .expect("failed to compute checksum");

    // Write the TCP header after the checksum calculation as it's now complete with all required information.
    let mut unwritten = &mut buf[ip_header_ends_at..]; // Update the unwritten reference to start at the end of the IP header.
    self.tcp.write(&mut unwritten); // Write the TCP header into the designated space in the buffer.

    // After filling the buffer with the IP header, TCP header, and payload, we send it via the 'nic' interface.
    let write_len = nic.send(&buf[..payload_ends_at])?;

    // The 'send' method on 'nic' returns the number of bytes written to the network interface, 
    // which should match the amount of data we wanted to send (from IP header start to the end of the payload).
    // This count is important for tracking successful data transmission.

    // Return the size of the payload actually written as the result of this method.
    Ok(payload_bytes)
}

```

With the write method in place we can go ahead and replace the logic of writing to buffer with a single call to the write function like this `c.write(nic, c.send.nxt, 0)?;`


### Sequence Number Validation
The first thing the `on_packet` method does is sequence number validation, the `on_packet` method treats the [sequence] numbers as positions on a circle defined by the TCP sequence space. To avoid the complications that would arise from number overflow, we use Rust’s `wrapping_add` function, allowing the sequence to gracefully roll over to 0 after hitting the 32-bit integer ceiling. This mechanism is much like an odometer rolling over after reaching its maximum display limit, ensuring mileage tracking continues smoothly without glitch.

The utility of wrapping arithmetic is fundamental to maintaining the TCP protocol's reliability. In our metaphorical number circle, "wrapping" denotes the  transition from the upper boundary to the start, facilitating a never-ending loop. Transposed to TCP's 32-bit context, this circle balloons to accommodate over four billion possible numbers. Rust's `wrapping_add` brings about this cyclical logic to sequence numbers, allowing TCP to keep a coherent narrative of bytes sent and received, irrespective of the actual sequence numbers, ensuring continuity and accuracy.

To grasp the relevance of this in TCP's domain, one must appreciate how sequence numbers can represent both position and history. As packets travel through networks which are inherently unreliable, they may arrive shuffled or delayed. The wrapping arithmetic preserves the integrity of this order, treating the sequence space like a clock where '1' comes after '12' as naturally as '11' does. It's this reliability in sequence number comparison that upholds TCP's promise of ordered, lossless data transmission.

>**From the RFC on Sequence Numbers**
>
>A fundamental notion in the design is that every octet of data sent over a TCP connection has a sequence number.  Since every octet is sequenced, each of them can be acknowledged.
>The acknowledgment mechanism employed is cumulative so that an acknowledgment of sequencenumber X indicates that all octets up to but not including X have been received.  This mechanism allows for straight-forward duplicate detection in the presence of retransmission.  Numbering of octets within a segment is that the first data octet immediately following the header is the lowest numbered, and the following octets are numbered consecutively.

### Valid Segment Check
The "valid segment check" is another pillar of TCP's reliable delivery guarantee. It ensures that the segment received is something the receiver is expecting as part of the current session. A segment is considered valid if its sequence number is within the window that the receiver expects new data to arrive. For a zero-length segment, the rules differ slightly; it is acceptable if it falls within the receive window or if it's the next expected sequence number when the window is full.

In the `on_packet` method, the received packet's sequence number is checked against the expected range using the `is_between_wrapped` function, which employs wrapping arithmetic to determine whether the sequence number falls within the expected window, accounting for wrapping. This is especially important for networks with high latency or when transmitting large amounts of data rapidly, as sequence numbers can wrap around frequently.

Once a valid segment is confirmed, and if the segment is an acknowledgment (ACK), the method proceeds to adjust the state of the connection accordingly. If in the "SynRcvd" state and the ACK is acceptable, the connection state changes to "Estab" (established). If the connection is already established, and the segment is acknowledging something new, the `send.una` (unacknowledged sequence number) is updated, and the connection proceeds to termination by sending a FIN segment if it's in the "Estab" state, transitioning to "FinWait1."

Furthermore, when a FIN segment is received while in "FinWait2," indicating the other end has finished sending data, the connection state transitions to "TimeWait," concluding the connection termination phase. This graceful termination ensures that both ends have a chance to confirm the reception of all transmitted data before completely closing the connection.

**Aside** <br>

A FIN segment is a type of TCP packet used to signal the sender has finished sending data. TCP is a bidirectional communication protocol, which means that each direction must be shut down independently. When one end of the connection has no more data to send, it sends a FIN segment, which the other end acknowledges. Only after both ends have exchanged and acknowledged FIN segments can the connection be considered fully closed. This process ensures that both parties have the opportunity to complete their data transmission fully before the connection is terminated.

**step by step walkthrough on the  `on_packet` Method checks:**

The `on_packet` method within our implementation is where the rubber meets the road regarding packet processing. When this method is called, the first order of business is to perform a series of checks:

1. **Sequence Number Validity**: This is the very first check, ensuring that the segment falls within the window of what we expect to receive next. The segment's sequence number is compared to the receiver's expected sequence number, considering the wrapping.
    
2. **Segment Length and Window Calculation**: The method calculates the length of the segment and uses the wrapping arithmetic to determine if the segment's data fits within the established window—the section of the connection buffer space currently available for new data.
    
3. **Acceptable Ack Check**: Here, the method verifies whether the acknowledgment number in the received segment is one that it should act upon or ignore, based on what it has previously sent.
    
4. **Zero-Length Segment and Window Checks**: For segments that carry no data, there are specific rules that determine their acceptability based on the sequence number and the current window size.
    
5. **Connection State Transitions**: Based on the received ACK and the current state, the connection may transition between states—moving from "SYN received" to "established" upon receiving a valid ACK, or moving towards connection teardown by progressing to "FIN wait" states after a FIN has been sent and acknowledged.
    
6. **Data Reception**: If the segment is valid and the state is established, the method updates the sequence number to the next expected byte, preparing the connection to receive further data.
    
7. **Reset Handling with `send_rst`**: If an unacceptable segment is received or if there's a need to abort the connection for other reasons, the `send_rst` method generates a reset segment, using the correct sequence numbers and flags based on the connection's synchronization state and the incoming segment's details.

```Rust
/// Processes a TCP packet based on the current connection state and sequence numbers.
/// It's part of a larger TCP state machine implementation.
pub fn on_packet<'a>(
    &mut self,
    nic: &mut tun_tap::Iface, // A mutable reference to the network interface.
    iph: etherparse::Ipv4HeaderSlice<'a>, // The IPv4 header of the received packet.
    tcph: etherparse::TcpHeaderSlice<'a>, // The TCP header of the received packet.
    data: &'a [u8], // The payload data of the packet.
) -> io::Result<()> {
    // Sequence number validation as per RFC 793, Section 3.3.
    let seqn = tcph.sequence_number(); // The sequence number of the TCP segment.
    let mut slen = data.len() as u32; // Length of the data payload in bytes.
    // Adjust sequence length for FIN and SYN flags, which consume a sequence number each.
    if tcph.fin() {
        slen += 1; // FIN flag indicates end of data, consumes one sequence number.
    };
    if tcph.syn() {
        slen += 1; // SYN flag indicates start of sync, consumes one sequence number.
    };
    // Calculate the window end using wrapping addition to handle potential overflow.
    let wend = self.recv.nxt.wrapping_add(self.recv.wnd as u32); // The end of the receiver's window.
    
    // Check for the validity of the segment length and sequence number.
    let okay = if slen == 0 {
        // Rules for acceptance of zero-length segments.
        if self.recv.wnd == 0 {
            seqn == self.recv.nxt // Accept only if sequence number equals the next expected number.
        } else {
            // Accept if within window and sequence space.
            is_between_wrapped(self.recv.nxt.wrapping_sub(1), seqn, wend)
        }
    } else {
        // Non-zero length segment rules.
        if self.recv.wnd == 0 {
            false // Do not accept any data if window is closed.
        } else {
            // Check if segment falls within the receive window.
            is_between_wrapped(self.recv.nxt.wrapping_sub(1), seqn, wend) ||
            is_between_wrapped(self.recv.nxt.wrapping_sub(1), seqn.wrapping_add(slen - 1), wend)
        }
    };

    // Respond with an ACK with the correct sequence number if the segment is not acceptable.
    if !okay {
        // Send a response with an ACK for the expected sequence number.
        self.write(nic, self.send.nxt, 0)?;
        return Ok(());
    }

    // Update the next expected sequence number after accepting valid data.
    self.recv.nxt = seqn.wrapping_add(slen);

    // If the TCP header does not have an ACK, no further processing is necessary.
    if !tcph.ack() {
        return Ok(());
    }

    // Handling ACKs in different states
    let ackn = tcph.acknowledgment_number(); // The acknowledgment number from the TCP header.
    // If the connection is in 'SYN-RECEIVED' state and the ACK is valid, move to 'ESTABLISHED'.
    if let State::SynRcvd = self.state {
        if is_between_wrapped(self.send.una.wrapping_sub(1), ackn, self.send.nxt.wrapping_add(1)) {
            // Transition to the established state.
            self.state = State::Estab;
        } else {
            // If the ACK is not within the expected range, a reset might be sent here.
            // [RFC 793 requires a RST for an unacceptable ACK while in SYN-RECEIVED]
            // TODO: Implement RST sending as per RFC specification.
        }
    }

    // If the connection is already established or in a state waiting for a finalization,
    // it processes the acknowledgment accordingly.
    if let State::Estab | State::FinWait1 | State::FinWait2 = self.state {
        if !is_between_wrapped(self.send.una, ackn, self.send.nxt.wrapping_add(1)) {
            // If ACK is not valid, no action is taken.
            return Ok(());
        }
        // Update the least unacknowledged byte after a valid ACK is received.
        self.send.una = ackn;
        // TODO: Implement logic to handle new ACKs and sending out any data that has been queued but not yet sent.
        assert!(data.is_empty()); // Assert no data is present for these states as per this implementation.

        // Transition to initiate connection termination by sending a FIN.
        if let State::Estab = self.state {
            // Terminate the connection with a FIN.
            self.tcp.fin = true; // Set FIN flag to indicate no more data will be sent.
            self.write(nic, self.send.nxt, 0)?; // Send the FIN packet.
            self.state = State::FinWait1; // Move to the state waiting for the FIN acknowledgment.
        }
    }

    // In 'FIN-WAIT-1' state, check if the FIN has been acknowledged.
    if let State::FinWait1 = self.state {
        if self.send.una == self.send.iss + 2 {
            // Confirm the peer has acknowledged our FIN.
            self.state = State::FinWait2; // Transition to 'FIN-WAIT-2' state.
        }
    }

    // If a FIN is received, handle it based on the current state.
    if tcph.fin() {
        match self.state {
            // If in 'FIN-WAIT-2', the receipt of a FIN indicates the other side has finished sending.
            State::FinWait2 => {
                // Finalize the connection closure process.
                self.write(nic, self.send.nxt, 0)?; // Acknowledge the received FIN.
                self.state = State::TimeWait; // Enter 'TIME-WAIT' state.
                // TODO: Implement the TIME-WAIT state duration and closing procedures.
            }
            _ => { /* States not expecting a FIN do not process it here. */ }
        }
    }

    // All cases either lead to a state transition or a write operation (or both),
    // demonstrating the intended flow of TCP connection state management.
    Ok(()) // Indicate successful processing of the incoming TCP packet.
}

```

```Rust
fn wrapping_lt(lhs: u32, rhs: u32) -> bool {
    // From RFC1323:
    //     TCP determines if a data segment is "old" or "new" by testing
    //     whether its sequence number is within 2**31 bytes of the left edge
    //     of the window, and if it is not, discarding the data as "old".  To
    //     insure that new data is never mistakenly considered old and vice-
    //     versa, the left edge of the sender's window has to be at most
    //     2**31 away from the right edge of the receiver's window.
    lhs.wrapping_sub(rhs) > (1 << 31)
}

fn is_between_wrapped(start: u32, x: u32, end: u32) -> bool {
    wrapping_lt(start, x) && wrapping_lt(x, end)
}
```

```Rust
    fn send_rst(&mut self, nic: &mut tun_tap::Iface) -> io::Result<()> {
        self.tcp.rst = true;
        // TODO: fix sequence numbers here
        // If the incoming segment has an ACK field, the reset takes its
        // sequence number from the ACK field of the segment, otherwise the
        // reset has sequence number zero and the ACK field is set to the sum
        // of the sequence number and segment length of the incoming segment.
        // The connection remains in the same state.
        //
        // TODO: handle synchronized RST
        // 3.  If the connection is in a synchronized state (ESTABLISHED,
        // FIN-WAIT-1, FIN-WAIT-2, CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT),
        // any unacceptable segment (out of window sequence number or
        // unacceptible acknowledgment number) must elicit only an empty
        // acknowledgment segment containing the current send-sequence number
        // and an acknowledgment indicating the next sequence number expected
        // to be received, and the connection remains in the same state.
        self.tcp.sequence_number = 0;
        self.tcp.acknowledgment_number = 0;
        self.write(nic, self.send.nxt, 0)?;
        Ok(())
    }
```

### Testing It Out
With `on_packet`, `write` and `send_rst` in place, our stack can now carry a connection all the way from the first SYN to TIME-WAIT. Let's see it in action. Just like in the previous chapters, we start our program with the `run.sh` script from chapter 1, which builds the binary, grants it `cap_net_admin`, and brings up `tun0` with the address `192.168.0.1/24`. Then, in a second terminal, we start capturing packets on our interface:

```
tshark -i tun0
```

And in a third terminal, we use Netcat to open a connection to an address inside our subnet. Any port will do, since we still accept a SYN on every port (we'll fix that in the next chapter):

```
nc 192.168.0.2 80
```

The exact output depends on your kernel, your Netcat and timing, so take the capture below as an example of what you should see rather than a literal recording. With our ISS of 0 (and tshark showing relative sequence numbers for the client), it should look something like this:

```
1   0.000000   192.168.0.1 -> 192.168.0.2   TCP 60 51234->80 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM=1 TSval=3155421 TSecr=0 WS=128
2   0.000187   192.168.0.2 -> 192.168.0.1   TCP 40 80->51234 [SYN, ACK] Seq=0 Ack=1 Win=10 Len=0
3   0.000231   192.168.0.1 -> 192.168.0.2   TCP 40 51234->80 [ACK] Seq=1 Ack=1 Win=64240 Len=0
4   0.000402   192.168.0.2 -> 192.168.0.1   TCP 40 80->51234 [FIN, ACK] Seq=1 Ack=1 Win=10 Len=0
5   0.000889   192.168.0.1 -> 192.168.0.2   TCP 40 51234->80 [FIN, ACK] Seq=1 Ack=2 Win=64240 Len=0
6   0.001013   192.168.0.2 -> 192.168.0.1   TCP 40 80->51234 [ACK] Seq=2 Ack=2 Win=10 Len=0
```

Notice that this time there is no router chatter before the handshake: everything is plain IPv4 between the kernel's end of `tun0` (`192.168.0.1`) and our user-space stack, which answers for `192.168.0.2`. Let's walk through it packet by packet:

1. **SYN**: Netcat asks the kernel to connect, and the kernel sends a SYN. There is no entry for this quad in our `connections` map yet, so `main.rs` hands it to `accept`. We record the client's sequence number as `recv.irs`, set `recv.nxt` to one past it (the SYN consumes a sequence number), and enter `SynRcvd`.
2. **SYN-ACK**: still inside `accept`, we set the SYN and ACK flags and call `write` with `send.nxt`. The segment carries `Seq=0` (our ISS) and `Ack=1`, and the `Win=10` is the tiny window we hard-coded. Because the SYN flag is set, `write` accounts for it in the sequence space, so our next sequence number becomes 1.
3. **ACK**: the client completes the handshake. This time the quad is in the map, so the segment goes to `on_packet`. It is zero-length and sits right at `recv.nxt`, so it passes the valid segment check. Its acknowledgment number (1) falls between `send.una - 1` and `send.nxt + 1`, so we move from `SynRcvd` to `Estab`. The connection is now established.
4. **FIN, ACK**: we don't wait around for long. In the same call to `on_packet`, the `Estab | FinWait1 | FinWait2` block sees that the ACK acknowledges something new (our SYN), so it moves `send.una` up to 1. Since we are in `Estab`, it sets `self.tcp.fin = true`, calls `write` with `Seq=1`, and moves us to `FinWait1`. We have nothing to say, so we hang up right away. The FIN consumes sequence number 1, so our next sequence number becomes 2.
5. **FIN, ACK** from the client: the kernel acknowledges our FIN (`Ack=2`). Netcat sees end-of-file and closes its side too, so the kernel's FIN usually rides along on the same segment. Back in `on_packet`, `slen` is 1 because of the FIN, so `recv.nxt` moves to 2. The ACK is new, so `send.una` becomes 2. Our FIN was the second sequence number we used (SYN first, then FIN), so `send.una == send.iss + 2` holds and we move from `FinWait1` to `FinWait2`. Then, since the segment also carries a FIN, the `match` on our state finds us in `FinWait2`.
6. **ACK**: we acknowledge the client's FIN by calling `write` one last time (`Seq=2`, `Ack=2`) and enter `TimeWait`. Both directions are now shut down.

While this happens, the terminal running our binary prints the debug output from `write`. Remember that the `ack` value is relative (`self.recv.nxt - self.recv.irs`), while `seq` is the absolute sequence number we pass in. With our ISS of 0, the two look alike. You should see something along these lines:

```
write(ack: 1, seq: 0, limit: 0) syn true fin false
using offset 0 base 0 in ([], [])
write(ack: 1, seq: 1, limit: 0) syn false fin true
using offset 0 base 1 in ([], [])
write(ack: 2, seq: 2, limit: 0) syn false fin false
using offset 0 base 2 in ([], [])
```

One line pair per segment we sent: the SYN-ACK, our FIN, and the final ACK. The unacked queue is empty every time (`([], [])`), because we haven't sent any data yet, which is exactly why `limit` is always 0.

**Aside** <br>

Not every client folds its ACK and its FIN into a single segment. If Netcat keeps its side open (some versions wait for you to press Ctrl-D or Ctrl-C), or the kernel acknowledges our FIN before the application closes, you'll see a bare `[ACK] Seq=1 Ack=2` first and the client's `[FIN, ACK]` some time later. The bare ACK moves us to `FinWait2` as expected. But look closely at what happens to the FIN that follows: its acknowledgment number is still 2, which equals `send.una`, and our "is this ACK new?" check uses the strict `is_between_wrapped(self.send.una, ackn, ...)`. That returns early, so we never reach the FIN handling and never send that last ACK. The client will keep retransmitting its FIN. This is a nice reminder that a duplicate ACK is not an invalid segment. It's one of the rough edges we'll smooth out when we revisit ACK processing.

### Loose Ends
We have left a fair number of `TODO`s in this chapter's code. That was on purpose: we wanted the full lifecycle of a connection working end to end before polishing every corner of RFC 793. Here are the ones to keep in mind:

* **RST sequence numbers**: `send_rst` currently zeroes out the sequence and acknowledgment numbers. The RFC wants the reset to take its sequence number from the incoming segment's ACK field (or be zero with the ACK set to the segment's sequence number plus its length), and wants synchronized states to answer an unacceptable segment with an empty ACK instead of a reset. We also don't send the RST that RFC 793 requires for an unacceptable ACK in `SynRcvd` yet.
* **TIME-WAIT duration**: we enter `TimeWait` and then stay there forever. A real stack waits for twice the maximum segment lifetime (2MSL) before deleting the TCB, so that a lost final ACK can still be retransmitted.
* **New ACKs and queued data**: when an ACK moves `send.una` forward, we should drop the acknowledged bytes and send anything that has been queued but not yet sent. For now we simply `assert!(data.is_empty())`, which means a client that actually sends us data will crash our program.
* **The unacked queue and `send_times`**: `write` already reads its payload from `self.unacked` and records timestamps in `self.timers.send_times`, but we haven't really put either one to work. They are the foundation for data transmission, round-trip time estimation and retransmission, and we'll come back to them in detail in later chapters.

What's next on our agenda? Let's take stock. In this chapter we sealed the three-way handshake and, for good measure, tore the connection right back down again. We added a `write` helper that builds and sends any segment we need, taught our sequence number comparisons to survive the 32-bit sequence space rolling over, and got `on_packet` walking the state machine from `SynRcvd` through `Estab`, `FinWait1` and `FinWait2` all the way to `TimeWait`. Seeing a clean handshake and FIN exchange show up in tshark, made of packets we built byte by byte, is a milestone worth pausing on.

However, if you have been paying attention, you'll notice our stack is still a little too eager. It answers a SYN on any port at all, and it hangs up on every connection the moment it opens. No application could actually use it yet. In Chapter 5 we'll fix that by listening on specific ports, modeling our interface on Rust's standard library `TcpListener` and `TcpStream` types, and managing several connections at the same time. Grab a coffee, because that is where we are headed next.

### References
- [Corresponding Code](https://github.com/jonhoo/rust-tcp/commit/fa1665f4141e205cde056d5eaa872933169e2fac)

