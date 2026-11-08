## The Listening Socket
Currently, our setup is such that whenever we receive any IPV4 SYN packet on any port, we are equipped to handle it. However, in a realistic server scenario, we want to listen on specific ports, handling SYN packets selectively to establish connections only where desired. In this chapter, we'll refine our approach to closely model the behavior of Rust's standard library's TCP constructs and ensure our server is capable of concurrent connection management.

If you remember where we left off, our stack is a little too eager. It shakes hands with anyone who knocks on any port, and then it immediately hangs up on them. That was fine for proving that the handshake and the teardown work, but no real program could build anything on top of it. Everything lives inside one big loop in `main.rs`, and there is no way for an application to say "I'm interested in port 8000, let me know when someone connects". By the end of this chapter, there will be.

### Where We're Headed
Before we write any code, let's look at what we want to end up with. If you've ever written a server in Rust, you've probably used `std::net::TcpListener`. You `bind` it to an address, call `accept` in a loop, and every call hands you a `TcpStream` connected to a new client. The operating system's TCP stack does all the hard work behind the scenes.

We are going to build the same shape of API on top of our own stack. Here is the `main.rs` we're working towards:

```Rust
// main.rs
use std::io;
use std::thread;

fn main() -> io::Result<()> {
    // Bring up tun0 and start the packet loop in the background.
    let interface = tcp::Interface::new()?;

    // Listen on two ports, each with its own thread accepting connections.
    let mut workers = Vec::new();
    for port in [8000, 9000] {
        let listener = interface.bind(port)?;
        workers.push(thread::spawn(move || {
            // Keep the streams around: we can't read from or write to them until the next chapter.
            let mut streams = Vec::new();
            while let Ok((stream, peer)) = listener.accept() {
                eprintln!(
                    "port {}: got a connection from {} (local end {})",
                    port,
                    peer,
                    stream.local_addr()
                );
                streams.push(stream);
            }
        }));
    }

    for worker in workers {
        worker.join().unwrap();
    }
    Ok(())
}
```

There is one piece here that you won't find in the standard library: the `Interface`. When you use `std::net`, the kernel owns the network interface and runs the TCP state machine for you, so all you ever see are sockets. Back in chapter 1 we chose to sit on the other side of the kernel-user space divide, which means we are the ones who have to own the interface and run the state machine. `Interface` is our handle to that: it opens `tun0`, processes every packet that comes in, and hands out listeners.

Notice also that `main` no longer knows anything about IP headers, quads or sequence numbers. All of that moves into a library, and `main.rs` becomes just another user of it.

### From Binary To Library
So far our project has been a single binary. To split it into a library and a program that uses it, all we need to do is add a `src/lib.rs` file. Cargo notices that both `src/lib.rs` and `src/main.rs` exist and builds a library crate and a binary crate, both named after the package. Our package is called `tcp` (remember `PKG_NAME=tcp` in `run.sh`), so `main.rs` can refer to the library as `tcp::Interface`, and `run.sh` keeps working without any changes.

The `mod tcp;` line moves from `main.rs` to the top of `lib.rs`, along with the `Quad` struct from chapter 3. Inside the library, our `tcp.rs` module and its `Connection` stay private: applications should only ever deal with `Interface`, `TcpListener` and `TcpStream`.

```Rust
// lib.rs
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

mod tcp;

// The connection quad from chapter 3, now living in the library.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
struct Quad {
    src: (Ipv4Addr, u16), // the remote end (who sent us the packet)
    dst: (Ipv4Addr, u16), // our end (the address and port they were trying to reach)
}
```

### Sharing State Between Threads
Here's the catch with the API we sketched above. `accept` blocks until a client connects, and in our example two different threads are blocked in `accept` at the same time. Meanwhile, somebody still has to sit in a loop reading packets off `tun0` and feeding them to the right connection. That can't happen on the application's threads, because they are busy waiting. So the packet loop gets a thread of its own.

Now we have several threads that all need to look at the same data. The packet loop needs the table of connections to find out where each packet belongs. The listeners need to know which connections are ready to be accepted. Let's put everything they need to agree on into a single struct:

```Rust
// lib.rs
// Everything the packet loop and the application threads need to agree on.
#[derive(Default)]
struct ConnectionTable {
    // Every connection we know about, exactly like the HashMap we had in main.rs.
    connections: HashMap<Quad, tcp::Connection>,
    // One entry per bound port. Each holds the connections that have finished
    // the handshake but haven't been picked up by `accept` yet (the backlog).
    listeners: HashMap<u16, VecDeque<Quad>>,
    // Set when the Interface is dropped, so the packet loop knows to stop.
    shutting_down: bool,
}

// The state shared between threads: the table behind a lock, plus a way to
// wake up anyone waiting for a new connection to show up.
#[derive(Default)]
struct Shared {
    table: Mutex<ConnectionTable>,
    new_connection: Condvar,
}
```

`connections` is the same `HashMap<Quad, Connection>` we have been using since chapter 3, it has just moved house. `listeners` is new. Its keys are the ports someone has bound to, and each value is a queue of connections on that port that have completed the three-way handshake but haven't been handed to the application yet. Real TCP stacks call this queue the *backlog*, and it's the reason a server can survive a burst of clients connecting faster than it calls `accept`.

To share this table between threads, we lean on three tools from `std::sync`:
* **`Arc`** (atomically reference counted) lets several threads own the same value. Every thread gets its own clone of the `Arc`, they all point at the same `Shared`, and the value is freed when the last clone goes away.
* **`Mutex`** makes sure only one thread at a time touches the `ConnectionTable`. Calling `lock()` gives us a guard, and the lock is released when the guard is dropped.
* **`Condvar`** (condition variable) lets a thread go to sleep until another thread tells it that something has changed. Our listeners will sleep on `new_connection` while their backlog is empty, and the packet loop will wake them up when a connection is ready.

If any of these are new to you, the [Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html) chapter of the Rust book is a great companion to this section. We'll only use the parts we need.

**Aside** <br>

You might be wondering whether one big lock around everything is a good idea. Every packet for every connection goes through the same `Mutex`, and so does every call to `accept`. For a real, high-throughput TCP stack the answer is no: you'd want finer-grained locking, per-connection state, or a different design altogether. For us, the single lock is a feature. It means there is exactly one place where shared state changes, which makes the stack much easier to reason about while we're still learning how all the pieces fit together.

### The Interface
With the shared state defined, the `Interface` itself turns out to be tiny. It holds its own clone of the `Arc<Shared>`, and its constructor does two things: it opens the TUN device, and it starts the packet loop on a new thread.

```Rust
// lib.rs
/// Our handle to the TUN device and the TCP stack running behind it.
pub struct Interface {
    shared: Arc<Shared>,
}

impl Interface {
    pub fn new() -> io::Result<Self> {
        let nic = tun_tap::Iface::without_packet_info("tun0", tun_tap::Mode::Tun)?;
        let shared = Arc::new(Shared::default());

        // The packet loop gets its own clone of the Arc and runs on its own thread,
        // so `new` can return straight away and the caller is free to bind and accept.
        let loop_shared = Arc::clone(&shared);
        thread::spawn(move || {
            if let Err(e) = packet_loop(nic, loop_shared) {
                eprintln!("packet loop stopped: {}", e);
            }
        });

        Ok(Interface { shared })
    }
}
```

Notice that the `nic` is *moved* into the packet loop's thread. From now on, the packet loop is the only code that ever touches the TUN device, so it doesn't need to be behind the lock at all. If the loop hits an I/O error, there's nobody waiting on the thread to hand the error to, so we log it.

When the application is done with the interface, we want the packet loop to stop. We implement `Drop` to flip the `shutting_down` flag:

```Rust
// lib.rs
impl Drop for Interface {
    fn drop(&mut self) {
        // Ask the packet loop to stop. It only checks this flag after it receives
        // a packet, so for now it may linger for a while (more on that later).
        self.shared.table.lock().unwrap().shutting_down = true;
    }
}
```

As the comment admits, this isn't perfect. The packet loop spends almost all of its time blocked inside `nic.recv`, and it only looks at the flag once a packet arrives. Fixing that properly means giving `recv` a timeout, which we'll need anyway once we start implementing TCP's timers. So we'll live with it for now.

### The Packet Loop
The packet loop is our old `main` loop with a new home. The parsing is the same as in the earlier chapters, but now that it lives in its own function, we can flatten the nested `match`es a little by using `continue` to skip packets we don't care about. The interesting changes are at the bottom, once we have the lock.

```Rust
// lib.rs
fn packet_loop(mut nic: tun_tap::Iface, shared: Arc<Shared>) -> io::Result<()> {
    let mut buf = [0u8; 1504];

    loop {
        let nbytes = nic.recv(&mut buf[..])?;

        // Parsing is the same as in the earlier chapters, just flattened a little.
        let iph = match etherparse::Ipv4HeaderSlice::from_slice(&buf[..nbytes]) {
            Ok(iph) => iph,
            Err(e) => {
                eprintln!("ignoring weird packet: {:?}", e);
                continue;
            }
        };
        if iph.protocol() != 0x06 {
            continue; // not TCP
        }
        let tcph = match etherparse::TcpHeaderSlice::from_slice(&buf[iph.slice().len()..nbytes]) {
            Ok(tcph) => tcph,
            Err(e) => {
                eprintln!("ignoring weird tcp packet: {:?}", e);
                continue;
            }
        };
        let datai = iph.slice().len() + tcph.slice().len();
        let quad = Quad {
            src: (iph.source_addr(), tcph.source_port()),
            dst: (iph.destination_addr(), tcph.destination_port()),
        };
        let local_port = quad.dst.1;

        // From here on we touch shared state, so we take the lock.
        let mut guard = shared.table.lock().unwrap();
        if guard.shutting_down {
            return Ok(());
        }
        // Reborrow the guard as a plain `&mut ConnectionTable` so we can borrow
        // `connections` and `listeners` separately below.
        let table = &mut *guard;

        match table.connections.entry(quad) {
            Entry::Occupied(mut c) => {
                let was_established = c.get().is_established();
                c.get_mut()
                    .on_packet(&mut nic, iph, tcph, &buf[datai..nbytes])?;

                // Did this packet complete the handshake? Then it's ready to be accepted.
                if !was_established && c.get().is_established() {
                    if let Some(backlog) = table.listeners.get_mut(&local_port) {
                        backlog.push_back(quad);
                        shared.new_connection.notify_all();
                    }
                    // TODO: if the listener went away mid-handshake, we should reset the connection.
                }
            }
            Entry::Vacant(e) => {
                // Only start a handshake if someone is actually listening on this port.
                if table.listeners.contains_key(&local_port) {
                    if let Some(c) =
                        tcp::Connection::accept(&mut nic, iph, tcph, &buf[datai..nbytes])?
                    {
                        e.insert(c);
                    }
                } else {
                    eprintln!("ignoring packet for port {}: nobody is listening", local_port);
                    // TODO: RFC 793 says we should answer with a RST here.
                }
            }
        }
        // `guard` is dropped here, releasing the lock for the application threads.
    }
}
```

Let's walk through what changed:

1. **Parsing before locking**: we parse the headers and build the quad *before* taking the lock. None of that touches shared state, and the less time we spend holding the lock, the less time the application threads spend waiting for it.

2. **The reborrow**: `let table = &mut *guard;` looks a bit odd. The `MutexGuard` gives us access to the `ConnectionTable`, but every field access through the guard goes through its `DerefMut` implementation, which borrows the *whole* guard. That means the borrow checker won't let us hold an `Entry` into `connections` while also looking things up in `listeners`. Turning the guard into a plain `&mut ConnectionTable` first lets Rust see that we are borrowing two different fields, which is perfectly fine.

3. **Only listening ports get a handshake**: in the `Vacant` arm, we used to call `Connection::accept` for any SYN that came our way. Now we first check that the destination port has an entry in `listeners`. If nobody has bound to it, we log the packet and move on. This is the fix for the "answers a SYN on any port" problem we pointed out at the end of the last chapter.

4. **Promoting established connections**: in the `Occupied` arm, we note whether the connection was established *before* handing it the packet. If the packet is what moved it into `Estab` (in other words, it was the client's ACK completing the handshake), we push the quad onto the listener's backlog and wake up anyone waiting in `accept`.

That last point is a deliberate choice. We could add the connection to the backlog as soon as we've sent our SYN-ACK, but then `accept` could hand the application a connection that might never finish its handshake. Waiting until the connection is fully established matches what you get from the operating system's `accept`, where every stream you receive is ready to use.

>**From the RFC on segments arriving for a closed port (section 3.9)**
>
>If the state is CLOSED (i.e., TCB does not exist) then all data in the incoming segment is discarded. An incoming segment containing a RST is discarded. An incoming segment not containing a RST causes a RST to be sent in response.

Strictly speaking, the RFC wants us to answer a SYN for a port nobody is listening on with a reset, which is how a client finds out straight away that the connection was refused. For now we just drop it, and the client will keep retrying until it gives up. We'll add the reset once we tidy up `send_rst`, which still has some TODOs of its own from chapter 4.

### No More Hanging Up
The packet loop relies on a small helper that we need to add to `Connection` in `tcp.rs`. The connection's `state` field is private to the `tcp` module, so we give the library a way to ask about it:

```Rust
// tcp.rs
impl Connection {
    /// Has this connection completed the three-way handshake?
    pub(crate) fn is_established(&self) -> bool {
        matches!(self.state, State::Estab)
    }
}
```

`pub(crate)` makes the method visible to the rest of our library, but not to applications using it.

There's one more change in `tcp.rs`, and it's a removal. In chapter 4, as soon as a connection became established, `on_packet` would set the FIN flag, send it and move to `FinWait1`. That gave us a nice clean teardown to look at in tshark, but it also means a connection would only ever be in `Estab` for a split second before we hung up. Deciding when to close a connection is the application's job, not the stack's. So we delete that block from `on_packet`:

```Rust
// tcp.rs, inside on_packet
if let State::Estab | State::FinWait1 | State::FinWait2 = self.state {
    if !is_between_wrapped(self.send.una, ackn, self.send.nxt.wrapping_add(1)) {
        return Ok(());
    }
    self.send.una = ackn;
    // TODO: Implement logic to handle new ACKs and sending out any data that has been queued but not yet sent.
    assert!(data.is_empty());

    // Delete this block: the stack no longer hangs up on its own.
    // if let State::Estab = self.state {
    //     self.tcp.fin = true;
    //     self.write(nic, self.send.nxt, 0)?;
    //     self.state = State::FinWait1;
    // }
}
```

Don't throw away the rest of the FIN handling though. The `FinWait1` and `FinWait2` logic is still correct, it just won't be reached until the application asks to close a stream. We'll wire that up when we implement closing a `TcpStream`.

### Binding To A Port
Binding is how an application tells the stack it wants to accept connections on a port. In our design, that boils down to inserting an empty backlog into `listeners`:

```Rust
// lib.rs
impl Interface {
    /// Start listening for connections on `port`.
    pub fn bind(&self, port: u16) -> io::Result<TcpListener> {
        let mut table = self.shared.table.lock().unwrap();
        if table.listeners.contains_key(&port) {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("port {} is already bound", port),
            ));
        }
        // An empty backlog: nobody has connected yet.
        table.listeners.insert(port, VecDeque::new());

        Ok(TcpListener {
            port,
            shared: Arc::clone(&self.shared),
        })
    }
}
```

Only one listener is allowed per port. If you try to bind a port that's already taken, you get back an `io::ErrorKind::AddrInUse` error, the same kind of error `std::net::TcpListener::bind` gives you when you try to start two servers on the same port. Since all the mutation happens behind the lock, `bind` only needs `&self`, so you can bind as many ports as you like from the same `Interface`.

### Accepting Connections
A `TcpListener` is just a port number and a clone of the shared state:

```Rust
// lib.rs
/// A socket bound to a port, waiting for connections.
pub struct TcpListener {
    port: u16,
    shared: Arc<Shared>,
}

impl TcpListener {
    /// Block until a connection on our port completes its handshake, then hand it over.
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddrV4)> {
        let mut table = self.shared.table.lock().unwrap();
        loop {
            let backlog = table
                .listeners
                .get_mut(&self.port)
                .expect("listener's port vanished from the table");

            if let Some(quad) = backlog.pop_front() {
                let peer = SocketAddrV4::new(quad.src.0, quad.src.1);
                let stream = TcpStream {
                    quad,
                    shared: Arc::clone(&self.shared),
                };
                return Ok((stream, peer));
            }

            // Nothing in the backlog yet. `wait` releases the lock while we sleep
            // and takes it again before returning.
            table = self.shared.new_connection.wait(table).unwrap();
        }
    }
}
```

If there's already a connection waiting in the backlog, `accept` pops it off the front and returns it straight away, along with the client's address, just like `std::net::TcpListener::accept` does. If the backlog is empty, we need to wait, and this is where the `Condvar` comes in.

`Condvar::wait` takes our `MutexGuard`, *releases the lock*, and puts the thread to sleep. When the packet loop calls `notify_all`, the thread wakes up, `wait` takes the lock again and hands us back a fresh guard. Releasing the lock while we sleep is crucial: the packet loop needs that very lock to put a connection in our backlog in the first place. If we held on to it while sleeping, nothing could ever wake us up.

Why the `loop`, though? There are a couple of reasons we might wake up and still find an empty backlog. We use `notify_all`, which wakes up *every* thread waiting on `new_connection`, including listeners on other ports whose backlogs haven't changed. And condition variables are allowed to have *spurious wakeups*, where `wait` returns even though nobody called notify. So every time we wake up, we check the backlog again, and go back to sleep if there's nothing for us.

You might also have noticed that the packet loop calls `notify_all` while it's still holding the lock. That's fine: the woken thread simply waits inside `wait` until the packet loop finishes processing the packet and drops its guard, and then it gets the lock.

**Aside** <br>

Yes, we now have two functions called `accept`. `Connection::accept` in `tcp.rs` is the one from chapter 3, which takes a SYN and starts a handshake. `TcpListener::accept` is the application-facing one, which hands over a connection once that handshake is done. They are two halves of the same idea: the stack accepts a connection at the TCP level, and later the application accepts it from the backlog. A real server can have plenty of connections sitting in the backlog that the stack has accepted, but the application hasn't yet.

When a listener is dropped, it unbinds its port:

```Rust
// lib.rs
impl Drop for TcpListener {
    fn drop(&mut self) {
        let mut table = self.shared.table.lock().unwrap();
        // Unbind the port. New SYNs to it will now be ignored.
        let _backlog = table.listeners.remove(&self.port);
        // TODO: connections still waiting in the backlog should be reset.
    }
}
```

### The Stream
Finally, the `TcpStream`. For now it is mostly an identity: the quad of the connection it belongs to and a clone of the shared state, so it can find its `Connection` in the table later.

```Rust
// lib.rs
/// One end of an established TCP connection.
pub struct TcpStream {
    quad: Quad,
    shared: Arc<Shared>,
}

impl TcpStream {
    /// The address and port of the other end of the connection.
    pub fn peer_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.quad.src.0, self.quad.src.1)
    }

    /// Our own address and port for this connection.
    pub fn local_addr(&self) -> SocketAddrV4 {
        SocketAddrV4::new(self.quad.dst.0, self.quad.dst.1)
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _table = self.shared.table.lock().unwrap();
        // TODO: send a FIN for self.quad and, once the connection is fully closed,
        // remove it from the table.
    }
}
```

A stream that can't send or receive anything isn't very useful yet, but that's coming next. Notice that the stream doesn't hold the `Connection` itself, only its quad. The `Connection` stays in the shared table, because the packet loop needs to keep updating it as packets arrive, whether or not the application is looking at it.

### Testing It Out
Time to take it for a spin. Replace your `main.rs` with the one from the start of the chapter and start the program with `run.sh` as usual. In a second terminal, start capturing on `tun0`:

```
tshark -i tun0
```

Then, in a third terminal, connect to one of the ports we're listening on:

```
nc 192.168.0.2 8000
```

This time, Netcat doesn't exit. The handshake completes and then everything goes quiet, because our stack no longer hangs up on its own. In the terminal running our program, you should see our `accept` loop pick up the connection:

```
port 8000: got a connection from 192.168.0.1:51234 (local end 192.168.0.2:8000)
```

Leave that Netcat running and open another terminal to connect to port `9000`. The second listener thread picks that one up independently, and both connections now sit in our table side by side. That's concurrent connection management, with the backlog and the `Condvar` doing the work behind the scenes.

Now try a port nobody is listening on:

```
nc 192.168.0.2 7000
```

Netcat will just sit there, and the capture should look something like this. As in chapter 4, take it as an example of what to expect rather than a literal recording; your ports and timings will differ.

```
1   0.000000   192.168.0.1 -> 192.168.0.2   TCP 60 51240->7000 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM=1 TSval=3160012 TSecr=0 WS=128
2   1.012345   192.168.0.1 -> 192.168.0.2   TCP 60 [TCP Retransmission] 51240->7000 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM=1 TSval=3161024 TSecr=0 WS=128
3   3.028901   192.168.0.1 -> 192.168.0.2   TCP 60 [TCP Retransmission] 51240->7000 [SYN] Seq=0 Win=64240 Len=0 MSS=1460 SACK_PERM=1 TSval=3163040 TSecr=0 WS=128
```

The client's SYNs go unanswered, and the kernel keeps retrying with longer and longer gaps in between. Meanwhile, our program logs `ignoring packet for port 7000: nobody is listening` for each one. Once we send a RST in this case, as the RFC wants, Netcat will instead fail immediately with "Connection refused".

Before you move on, try pressing Ctrl-C in one of the Netcat sessions connected to port 8000 or 9000. You'll see the client send a FIN, and you'll see it retransmitted over and over, because nothing in our stack handles a FIN in the `Estab` state yet. Hold that thought for the loose ends below.

### Loose Ends
We've added quite a few TODOs this time round. They are all things we'll come back to, but it's worth knowing where the gaps are:

* **A FIN from the client is ignored**: our FIN handling only covers the case where *we* closed first (`FinWait2`). When the client closes first, we should acknowledge its FIN and move to `CloseWait`, and the application should see the end of the stream.
* **Dropping things doesn't clean up yet**: dropping a `TcpStream` should send a FIN and eventually remove the connection from the table. Dropping a `TcpListener` should reset any connections still sitting in its backlog. Dropping the `Interface` only stops the packet loop when the next packet arrives, because `recv` has no timeout.
* **No resets for refused connections**: SYNs for ports nobody is listening on are silently dropped, and so are connections whose listener disappears mid-handshake. Both should get a RST.
* **Streams can't send or receive data**: there is no `read` or `write` on `TcpStream` yet, and `on_packet` still asserts that incoming segments are empty, so a client that actually sends us something will bring the whole packet loop down.

What's next on our agenda? Let's take stock. We turned our stack into a library, moved the packet loop onto its own thread, and shared the connection table between threads with an `Arc`, a `Mutex` and a `Condvar`. Applications can now `bind` to the ports they care about, `accept` connections from a backlog once their handshake is complete, and leave everything else to the stack. SYNs for ports nobody is listening on no longer get a reply at all.

However, if you have been paying attention, you'll notice that the streams we hand out are still empty shells. You can accept a connection, but you can't say anything over it, and the moment the client says something to you, our stack falls over. In the next chapter we'll fix that by implementing `Read` and `Write` for `TcpStream`, putting the `incoming` and `unacked` queues to work, and finally sending some actual data over our very own TCP connections.

### References
- [RFC 793, section 3.9: Event Processing](https://www.rfc-editor.org/rfc/rfc793#section-3.9)
- [std::net::TcpListener](https://doc.rust-lang.org/std/net/struct.TcpListener.html)
- [std::sync::Condvar](https://doc.rust-lang.org/std/sync/struct.Condvar.html)
- [The Rust Book: Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Jon's take on this step](https://github.com/jonhoo/rust-tcp/commit/759baab5e245ba859ff52d3ce85881961497ea4b): his approach differs from ours in a few places, so it's worth comparing notes
