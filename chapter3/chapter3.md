### The connection Quad

We have made quite some progress in our project - setting us up to easily tackle the TCP protocol implementation. We are going to be discussing some of the behaviours of TCP & its various states. 

In a multitasking OS, various applications—like web servers, email clients, and more—need simultaneous network access. To distinguish between these different network activities, each application binds itself to a unique port number. A combination of this port number, along with the host's IP address, forms(at a very basic level) what is known as a "socket" - a kernel managed abstraction. To establish a connection, a pair of these sockets is used—one for the sending machine and one for the receiving machine. A socket pair is unique and effectively identifies a connection.

-
A very important data structure associated with TCP connection endpoints Transmission Control Blocks (TCBs). TCBs serve as the central repository for all the parameters and variables that pertain to an established or pending connection. Containing everything from socket addresses and port numbers to sequence numbers and window sizes, the TCB is the cornerstone of TCP's operational logic.

Each TCB is an encapsulation of the TCP state for a particular connection. It stores details that are essential for the "reliable" in "Transmission Control Protocol," such as unacknowledged data, flow control parameters, and the next expected sequence number. In essence, the TCB functions as the control center for TCP operations, maintaining and updating the state of the connection in real-time. The TCB serves as the kernel's ledger for a TCP connection, storing everything from port numbers and IP addresses to flow control parameters and pending data. Essentially, the TCB is the repository for all the metrics and variables that a TCP stack needs to maintain to handle a connection reliably and effectively.

-
Now, consider a machine involved in multiple TCP connections. How does it differentiate between them & discern which incoming packet belongs to which connection? This is where the concept of the 'connection quad' becomes crucial. A connection quad consists of a tuple with four elements: source IP address, source port, destination IP address, and destination port. This tuple serves as a unique identifier for each active TCP connection. The brilliance of the connection quad lies in its utility as a unique key for indexing the hash table of TCBs. When a packet arrives, the TCP stack uses the connection quad to look up the corresponding TCB and, by extension, the relevant state machine. This ensures that the packet is processed according to the correct set of rules and variables, as defined by the state machine encapsulated in that TCB. TCP creates and maintains a hash table of transmission control blocks (TCBs) to store data for each TCP connection. A control block is attached to the table for each active connection. The control block is deleted shortly after the connection is closed.

-
A connection progresses through a series of states during its lifetime.  The states are:  LISTEN, SYN-SENT, SYN-RECEIVED, ESTABLISHED, FIN-WAIT-1, FIN-WAIT-2, CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT, and the fictional state CLOSED.  These states, embedded in the Transmission Control Block (TCB), serve as critical markers that guide the connection's behavior at each juncture. From the initial handshake to data transfer and eventual teardown, the state of a connection is meticulously tracked to facilitate reliable and ordered data exchange.

The initial states—`CLOSED` and `LISTEN`—serve as the connection's genesis points. When in the `CLOSED` state, no Transmission Control Block (TCB) exists, essentially making the connection non-existent. It's akin to an uninitialized variable; any packets received for this state are disregarded. Contrast this with the `LISTEN` state, where the system is actively waiting for incoming connection requests. Once a `SYN` packet is received, the state transitions to `SYN-RECEIVED`, initiating the TCP handshake. The connection reaches a steady `ESTABLISHED` state post-handshake, where most of the data exchange occurs.

As the connection concludes, it cascades through a series of `FIN-WAIT` and `CLOSE-WAIT` states, ultimately reaching `TIME-WAIT` or `LAST-ACK` depending on the teardown initiation. These terminal states ensure that any lingering packets in the network are accounted for, providing a graceful connection teardown. The system finally reverts to the `CLOSED` state, freeing the TCB and associated resources for future connections.

[IMAGE OF TCP DATA STATE - SEARCH RFC FOR 'ESTAB']
Enough of the talk, Lets write some code! 
To get started we are going to encode the state diagram using an Enum. To make things more modular we can create a new module called TCP and house some code in TCP.rs. The Enum & other logic related to the TCP state machine  will live there. 
```Rust
tcp.rs
// Defining possible TCP states. 
// Each state represents a specific stage in the TCP connection.
pub enum State { 
/// The connection is closed and no active connection exists. 
Closed, 
/// The endpoint is waiting for a connection attempt from a remote endpoint. 
Listen, 
/// The endpoint has received a SYN (synchronize) segment and has sent a SYN-ACK /// (synchronize-acknowledgment) segment in response. It is awaiting an ACK (acknowledgment) /// segment from the remote endpoint. 
SynRcvd, 
/// The connection is established, and both endpoints can send and receive data. Estab, 
}


// Implementing the Default trait for State. // Sets the default TCP state to 'Listen'. 
impl Default for State { 
	fn default() -> Self { 
		State::Listen 
	} 
}

// Implementing methods for State. 
impl State {
// Method to handle incoming TCP packets. 
// 'iph' contains the parsed IPv4 header, 'tcph' contains the parsed TCP header, and 'data' contains the TCP payload. 
pub fn on_packet<'a>( 
	&mut self, 
	iph: etherparse::Ipv4HeaderSlice<'a>, 
	tcph: etherparse::TcpHeaderSlice<'a>, 
	data: &'a [u8], 
	) { 
	// Log the source and destination IP addresses and ports, as well as the payload length. 
		eprintln!( 
			"{}:{} -> {}:{} {}b of TCP", 
			iph.source_addr(), 
			tcph.source_port(), 
			iph.destination_addr(), 
			tcph.destination_port(), 
			data.len() 
		); 
	} 
}
```

```Rust
main.rs
// Importing necessary modules and packages.
use std::io;
use std::collections::HashMap;
use std::net::Ipv4Addr;

// Defining the Quad struct that holds information about both the source and destination IP address and port.
// This uniquely identifies a TCP connection and will be used as a key in the HashMap.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct Quad {
    src: (Ipv4Addr, u16),
    dst: (Ipv4Addr, u16),
}

fn main() -> io::Result<()> {
    // Initialize a HashMap to store the TCP connection state against the Quad.
    // Quad is the key and the TCP state (from tcp.rs) is the value.
    let mut connections: HashMap<Quad, tcp::State> = Default::default();
    
    // Initialize the network interface.
    let nic = tun_tap::Iface::new("tun0", tun_tap::Mode::Tun)?;
    
    // Buffer to store the incoming packets.
    let mut buf = [0u8; 1504];

    loop {
    
			-----SKIP-----

        // Attempt to parse the IPv4 header.
        match etherparse::Ipv4HeaderSlice::from_slice(&buf[4..nbytes]) {
            Ok(iph) => {
					-----SKIP-----

                // Attempt to parse the TCP header.
                match etherparse::TcpHeaderSlice::from_slice(&buf[4 + iph.slice().len()..nbytes]) {
                    Ok(tcph) => {
                        // Calculate the start index of the actual data in the packet.
                        let datai = 4 + iph.slice().len() + tcph.slice().len();
                        
                        // Look for or create a new entry in the HashMap for this connection.
  
// Check if the connection already exists in the HashMap, otherwise create a new entry 
match connections.entry(Quad { 
src: (src, tcph.source_port()), 
dst: (dst, tcph.destination_port()), 
}) { 
Entry::Occupied(mut c) => { 
c.get_mut().on_packet(&mut nic, iph, tcph, &buf[datai..nbytes])?; 
} 
Entry::Vacant(e) => { 
if let Some(c) = tcp::Connection::on_accept(&mut nic, iph, tcph, &buf[datai..nbytes])? { 
e.insert(c); 
} } } }
                    }
                    Err(e) => {
                        // Handle TCP header parsing errors.
                        eprintln!("An error occurred while parsing the TCP packet: {:?}", e);
                    }
                }
            }
            Err(e) => {
                // Handle IPv4 header parsing errors.
                eprintln!("An error occurred while parsing the IP packet: {:?}", e);
            }
        }
    }
}

```

In the code, we establish the foundation of our TCP stack by defining the key data structures and functions needed for a TCP connection. Central to this architecture is the `Quad` struct, which serves as the unique identifier for each entry in a `HashMap` named `connections`. This HashMap acts as an in-memory registry for TCP connections that are either active or in the process of being established. Each entry in the HashMap consists of an instance of `Quad` mapped to its current TCP Connection. This state is represented by an enumeration type, defined in `tcp.rs`, embodying one of four TCP connection states: `Closed`, `Listen`, `SynRcvd`, or `Estab`.

We introduce two primary methods for packet handling: `on_accept` and `on_packet`. The `on_accept` method is responsible for handling incoming packets that initiate new connections. Conversely, the `on_packet` method manages packets for existing connections. Both methods log essential information such as source and destination IP addresses and ports, as well as the payload length. Finally, in `main.rs`, we make use of pattern matching to differentiate between new and existing connections, based on the incoming packets.

We're making steady progress. Thus far, we've ensured that we're receiving the correct IPv4 packets, and we've implemented a mechanism to associate incoming packets with their respective states, keyed by a unique connection quad. Our next objective is to focus on implementing TCP handshakes, a critical step for facilitating a dependable connection between a client and a server. The client initiates this process by sending a SYN (Synchronize) packet, while the server listens for these incoming requests. This handshake is a three-stage procedure involving a SYN packet, followed by a SYN-ACK (Synchronize-Acknowledgment) packet, and concluding with an ACK (Acknowledgment) packet. During this phase, we'll enhance the `accept` method to manage these distinct types of handshake packets.
```
CODE BLOCK

// Required imports
use std::io;
mod tcp;  // Importing the tcp module (defined below)
use std::collections::HashMap;
use std::net::Ipv4Addr;

// Define the Quad struct to represent the 4-tuple of source IP, source port, destination IP, and destination port.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct Quad {
    src: (Ipv4Addr, u16),  // Source IP and port
    dst: (Ipv4Addr, u16),  // Destination IP and port
}

fn main() -> io::Result<()> {
    // Create a hashmap to manage TCP connections
    let mut connections: HashMap<Quad, tcp::Connection> = Default::default();

    // Create a new virtual interface in TUN mode.
    // TUN mode allows to work with IP packets directly, while TAP mode works at Ethernet frame level.
    let mut nic = tun_tap::Iface::without_packet_info("tun0", tun_tap::Mode::Tun)?;
    
    // Buffer to hold the packet data
    let mut buf = [0u8; 1504];
    
    // Main processing loop to handle incoming packets
    loop {
        // Read the packet into the buffer
        let nbytes = nic.recv(&mut buf[..])?;

        // Parse the IP header from the packet. If successful, it gives a slice of the IP header.
        match etherparse::Ipv4HeaderSlice::from_slice(&buf[..nbytes]) {
            Ok(iph) => {
                // Parsing of the IP header was successful. Further processing happens here.
                // -----SKIP----- (omitting some IP processing steps as indicated)

                // Attempt to parse the TCP header from the packet.
                match etherparse::TcpHeaderSlice::from_slice(&buf[iph.slice().len()..nbytes]) {
                    Ok(tcph) => {
                        // Parsing of the TCP header was successful. Continue processing.
                        // Determine the index where the actual data starts in the packet (after IP and TCP headers).
                        let datai = iph.slice().len() + tcph.slice().len();
                        
                        // Lookup the connection using the Quad (4-tuple) as the key.
                        match connections.entry(Quad {
                            src: (src, tcph.source_port()),
                            dst: (dst, tcph.destination_port()),
                        }) {
                            // If a connection already exists for this Quad:
                            Entry::Occupied(mut c) => {
                                c.get_mut()
                                    .on_packet(&mut nic, iph, tcph, &buf[datai..nbytes])?;
                            }
                            // If there's no connection yet for this Quad:
                            Entry::Vacant(mut e) => {
                                // Attempt to establish a new connection.
                                if let Some(c) = tcp::Connection::accept(
                                    &mut nic,
                                    iph,
                                    tcph,
                                    &buf[datai..nbytes],
                                )? {
                                    e.insert(c);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Handle TCP parsing errors.
                        eprintln!("An error occurred while parsing TCP packet {:?}", e);
                    }
                }
            }
            Err(e) => {
                // Handle IP parsing errors.
                eprintln!("An error occurred while parsing IP packet {:?}", e);
            }
        }
    }
}

```

```
tcp.rs
use std::io;
use std::io::prelude::*;

/// The possible states a TCP connection can be in, based on the TCP state machine.
/// It's a subset of the states available in the full TCP state diagram.
pub enum State {
    Closed,
    Listen,
    SynRcvd,
    //Estab,
}

// TODO: It seems like more states might be added in the future based on the commented-out line.
// Consider extending this to cover all states.

pub struct Connection {
    /// The current state of the TCP connection.
    state: State,
    /// The sequence space for sent data. It keeps track of various sequence numbers for data we've sent.
    send: SendSequenceSpace,
    /// The sequence space for received data. It keeps track of sequence numbers for data we're receiving.
    recv: RecvSequenceSpace,
    ip: etherparse::Ipv4Header
    tcp: etherparse::TcpHeader
}

/// Representation of the Send Sequence Space as described in RFC 793 Section 3.2 Figure 4.
/// It provides a visual representation of various sequence numbers associated with data that's being sent.
///
/// ```
///            1         2          3          4
///       ----------|----------|----------|----------
///              SND.UNA    SND.NXT    SND.UNA
///                                   +SND.WND
///
/// 1 - Old sequence numbers which have been acknowledged by the receiver.
/// 2 - Sequence numbers of unacknowledged data that has been sent.
/// 3 - Sequence numbers allowed for transmitting new data.
/// 4 - Future sequence numbers that are not allowed for transmission yet.
/// ```
struct SendSequenceSpace {
    /// SND.UNA: Oldest sequence number not yet acknowledged by the receiver.
    una: u32,
    /// SND.NXT: Next sequence number to be used for new data for transmission.
    nxt: u32,
    /// SND.WND: The window size or the number of bytes that are allowed to be outstanding (unacknowledged).
    wnd: u16,
    /// Indicates if the URG control bit is set. If true, then the sequence number in the urgent pointer field is in effect.
    up: bool,
    /// Sequence number of the segment used for the last window update. 
    wl1: usize,
    /// Acknowledgment number used for the last window update.
    wl2: usize,
    /// Initial send sequence number. It's the first sequence number used when the connection was established.
    iss: u32,
}

/// Representation of the Receive Sequence Space as described in RFC 793 Section 3.2 Figure 5.
/// It provides a visual representation of sequence numbers associated with data that's being received.
///
/// ```
///                1          2          3
///            ----------|----------|----------
///                   RCV.NXT    RCV.NXT
///                             +RCV.WND
///
/// 1 - Old sequence numbers which have been acknowledged.
/// 2 - Sequence numbers allowed for receiving new data.
/// 3 - Future sequence numbers which are not allowed for reception yet.
/// ```
struct RecvSequenceSpace {
    /// RCV.NXT: Next expected sequence number that the receiver is expecting.
    nxt: u32,
    /// RCV.WND: The number of bytes that the receiver is willing to accept.
    wnd: u16,
    /// Indicates if the URG control bit is set on received data.
    up: bool,
    /// Initial receive sequence number. The sequence number of the first byte received.
    irs: u32,
}

/// Default state for a TCP connection is set to `Listen`.
impl Default for State {
    fn default() -> Self {
        State::Listen
    }
}

impl Connection {
    /// Handles an incoming TCP packet for establishing a connection.
    /// If the incoming packet is a SYN packet, it prepares and sends a SYN-ACK packet in response.
    /// Otherwise, it ignores the packet.
    ///
    /// Parameters:
    /// - `nic`: The network interface to use for sending the SYN-ACK packet.
    /// - `iph`: The IPv4 header of the incoming packet.
    /// - `tcph`: The TCP header of the incoming packet.
    /// - `data`: The payload of the incoming packet.
    ///
    /// Returns:
    /// A new `Connection` in the `SynRcvd` state if the incoming packet was a SYN packet.
    /// Otherwise, returns `None`.
    pub fn accept<'a>(
        nic: &mut tun_tap::Iface,
        iph: etherparse::Ipv4HeaderSlice<'a>,
        tcph: etherparse::TcpHeaderSlice<'a>,
        data: &'a [u8],
    ) -> io::Result<Option<Self>> {
        let mut buf = [0u8; 1500];
        if !tcph.syn() {
            // Ignore packets that aren't SYN packets.
            return Ok(None);
        }
        let iss = 0;
        let wnd = 10;
        let mut c = Connection {
            state: State::SynRcvd,
            send: SendSequenceSpace {
                iss,
                una: iss,
                nxt: 1,
                wnd: wnd,
                up: false,
                wl1: 0,
                wl2: 0,
            },
            recv: RecvSequenceSpace {
                // Initialize the receive sequence number to the incoming sequence number.
                irs: tcph.sequence_number(),
                // Expect the next byte after the incoming sequence number.
                nxt: tcph.sequence_number() + 1,
                // Use the incoming packet's window size for our receive window.
                wnd: tcph.window_size(),
                up: false,
            },
        // TODO: Consider keeping track of sender info for future use.
        // Prepare a SYN-ACK packet in response to the SYN packet.
        tcp: etherparse::TcpHeader::new(
            tcph.destination_port(),
            tcph.source_port(),
            iss,
            wnd,
		),
            ip: etherparse::Ipv4Header::new(
            syn_ack.header_len(),
            64,
            etherparse::IpNumber::Tcp as u8,
            [
                iph.destination()[0],
                iph.destination()[1],
                iph.destination()[2],
                iph.destination()[3],
            ],
            [
                iph.source()[0],
                iph.source()[1],
                iph.source()[2],
                iph.source()[3],
            ],
        )
        };

        c.tcp.acknowledgment_number = c.recv.nxt;
        c.tcp.syn = true;
        c.tcp.ack = true;
        
        c.ip.set_payload_len(c.tcp.header_len() as usize + 0);

        // Calculate and set the checksum for the SYN-ACK packet.
        syn_ack.checksum = syn_ack
            .calc_checksum_ipv4(&c.ip, &[])
            .expect("Failed to compute checksum");
        
        // Write out the TCP and IP headers to a buffer to be sent.
        let unwritten = {
            let mut unwritten = &mut buf[..];
            ip.write(&mut unwritten);
            syn_ack.write(&mut unwritten);
            unwritten.len()
        };

        // Send the SYN-ACK packet.
        nic.send(&buf[..unwritten])?;
        Ok(Some(c))
    }

    /// TODO: Implement a function to handle incoming packets for an established connection.


    // Function to handle incoming packets once a connection is established.
    pub fn on_packet<'a>(
        &mut self, 
        nic: &mut tun_tap::Iface,                  // The network interface
        iph: etherparse::Ipv4HeaderSlice<'a>,       // The parsed IP header
        tcph: etherparse::TcpHeaderSlice<'a>,       // The parsed TCP header
        data: &'a [u8],                            // The actual data from the packet
    ) -> io::Result<()> {
        // Process the packet based on its flags and the current state of the connection.
        // The code is omitted, but would involve handling the different possible states and flags 
        // (e.g., ACK, FIN, RST) as per the TCP state machine.
        // ----SKIP----
        Ok(())
    }

    // Function to send a SYN-ACK packet.

}

```

  
In transitioning from the code containing the skeleton of our implementation to the fleshed out version where we establish a TCP handshake, we've undergone a significant architectural evolution. Central to this architecture is the extension of the `State` enum and the `Connection` struct in `tcp.rs`. Previously, our `State` enum was a simple representation of four possible TCP states. In the new implementation, the focus shifts to establishing a more robust representation of connection states. The `Connection` struct has been extended to now hold instances of `SendSequenceSpace` and `RecvSequenceSpace` as its attributes. These structs are instrumental in the bookkeeping of TCP's sequence and acknowledgment numbers, which are paramount for the reliable delivery of data. Let's delve into the specifics.

The `SendSequenceSpace` struct encapsulates several variables crucial for the sending side of our TCP connection. Key among them are:

- `una` (send unacknowledged): The earliest sequence number in the send buffer that has yet to receive an acknowledgment.
- `nxt` (send next): The next sequence number to be used for new data.
- `wnd` (send window): Specifies the range of acceptable sequence numbers for the receiver. These fields are critical in managing flow control, ensuring reliable data transfer, and implementing features like sliding windows.

Complementing this, the `RecvSequenceSpace` struct handles the receiving end. It possesses fields like:

- `nxt` (receive next): The sequence number expected for the next incoming packet.
- `wnd` (receive window): The window size advertising the range of acceptable sequence numbers for the sender.

It is important to note that both `SendSequenceSpace` and `RecvSequenceSpace` contain initial sequence numbers (`iss` and `irs` respectively). These are pivotal during the connection establishment phase, specifically during the TCP three-way handshake process.

We set out to implement the `accept` method, This method has now been augmented to handle SYN packets, thus initiating the three-way handshake by dispatching a SYN-ACK packet back to the client. To achieve this, we employ etherparse library functions for constructing and sending the TCP/IP headers, aligning our implementation closely with the protocol's specifications.

As an aside - it is important to note that in the current implementation, the `connections` HashMap is vulnerable to a SYN flood attack. In such an attack, an adversary could send a large number of TCP SYN (synchronization) packets, each with a different source address and port, but all targeting the same destination. Since the `connections` HashMap automatically creates a new entry for each unique `Connection`, an attacker could easily exhaust the system's memory by populating the HashMap with a multitude of fake entries. This could lead to resource exhaustion and, ultimately, a denial of service (DoS).

In production-grade TCP implementations, measures are usually put in place to mitigate such risks. This can include using syn cookies - a stateless method where the server does not allocate resources for a SYN request until the handshake is completed. However, for this project we will not be taking any steps to prevent The syn flood attack.

At this point you are probably curious and see the long lines of code we have written in action. We should go ahead and test our application. First, We will start our program by running the `run.sh` script which will build & execute our binary and give it the necessary elevated network access, next we will use netcat to attempt to establish a TCP connection with our application and finally to visualize things we will use tshark to monitor and capture packets on our tun0 interface by running `tshark -I tun0`. 
```
1   0.000000   fe80::a2b3:c4d5:e6f7 -> ff02::2  ICMPv6 110 Router Solicitation from a2:b3:c4:d5:e6:f7
2   0.002123   fe80::1:1 -> fe80::a2b3:c4d5:e6f7  ICMPv6 150 Router Advertisement from 00:11:22:33:44:55 (MTU: 1500)
3   0.004567   fe80::a2b3:c4d5:e6f7 -> fe80::1:1   TCP 86 51234->80 [SYN] Seq=0 Win=42800 Len=0 MSS=1440 WS=16 SACK_PERM=1
4   0.006789   fe80::1:1 -> fe80::a2b3:c4d5:e6f7   TCP 86 80->51234 [SYN, ACK] Seq=0 Ack=1 Win=64000 Len=0 MSS=1440 SACK_PERM=1 WS=16
5   0.006999   fe80::a2b3:c4d5:e6f7 -> fe80::1:1   TCP 66 51234->80 [ACK] Seq=1 Ack=1 Win=43000 Len=0

```
If everything goes well you should see sth similar to what we see above. This shows that our host first asks for router information, then gets a reply from a router. After that, we see the steps of a TCP connection being made.

What's next on our agenda? If you have been paying attention, you'll notice we've tackled the initial two steps of the TCP three-way handshake. When a SYN from the client comes our way, our server promptly shoots back with a SYN-ACK. After dispatching that SYN-ACK, the server gracefully steps into the `SynRcvd` state, poised and ready for the client's ACK to seal the handshake deal. The moment we capture and process this ACK, we'd ideally transition our connection into the `Established` state, signaling the birth of a full-fledged TCP connection. However, there's a piece of the puzzle missing here: our code is still in the waiting room, yet to handle the client's ACK. And that, my friends, is our next port of call.
