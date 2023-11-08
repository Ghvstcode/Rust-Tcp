## The Final Handshake
Moving on to the `on_packet` method, we step into a crucial juncture in the TCP three-way handshake. After sending a SYN-ACK, the server engages this method as it awaits the client's ACK. This acknowledgment propels the connection from the "SYN-RECEIVED" state into the "ESTABLISHED" one, cementing the handshake and formally opening the communication channel. The `on_packet` method does the heavy lifting of finalizing the three-way handshake
It checks for the ACK response from the client. If the ACK is correctly received, the method will shift the connection's status to "ESTABLISHED," signifying that the TCP connection is complete and data transfer can begin.

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

TODO - CONCLUDE THIS CHAPTER, SHOW THE RESULTS OF RUNNING THIS CODE
