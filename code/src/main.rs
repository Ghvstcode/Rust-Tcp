//! The program from the start of chapter 5: listen on ports 8000 and 9000 and
//! log every connection that completes its handshake.
//!
//! Needs Linux, a TUN device and `cap_net_admin`; see code/README.md for the
//! run script from chapter 1. Part of the skeleton, in no task's write scope.

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
            // Keep the streams around: we can't read from or write to them yet (chapter 6).
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
        if worker.join().is_err() {
            eprintln!("a listener thread panicked");
        }
    }
    Ok(())
}
