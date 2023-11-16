## The Kernel-User Space Divide
The task of building our own TCP stack presents a unique set of challenges. Normally, when our user-space applications require internet connectivity, they make calls to high-level APIs provided by the operating system's kernel. These APIs facilitate the creation, sending, and receiving of data over network connections, abstracting away the complexities of raw packet handling. It's a fantastic arrangement for developing standard applications.

However, things get tricky when you're aiming to construct a custom TCP stack. With your own custom TCP stack, you're not just a consumer of network services; you have to be the manager, the processor, and the dispatcher. That means you need to interact directly with raw network packets, process them, and then send them off to their respective destinations. In essence, you'll have to bypass the operating system's built-in TCP stack to directly receive and process raw packets from the internet within your user-space TCP stack.

To allow us to achieve our sub-goal of handling raw network packets directly[in the user space], we're going to set up a virtual network interface. The virtual network interface will "trick" the kernel into passing incoming packets directly to it, just like a physical NIC (Network Interface Card), but without the kernel meddling in the raw packet processing. For this little trick, we'll employ the Linux TUN/TAP device driver, zeroing in on the TUN (Network) side of things to spin up our virtual network interface.

At its core - a TUN device is a [virtual] software-based network interface that exists in the operating system's kernel. This virtual network interface behaves much like a physical network interface, but it is not tied to a physical piece of hardware. The TUN device operates at Layer 3 of the OSI model and exposes a file descriptor to any applications that need to send or receive packets. 

Once we've got our TUN device up and running, any packet aimed at its associated IP address will be redirected by the kernel—no questions asked, no packet processed—straight into the lap of the user-space application that has bound itself to the TUN device. This setup gives us the carte blanche we need to fiddle with raw packets to our heart's content.

### Packet Handling Workflow: TUN Device vs Standard Network Stack


| Step  | With TUN Device                          | Without TUN Device                     |
|-------|------------------------------------------|----------------------------------------|
| 1     | Packet arrives at physical NIC.          | Packet arrives at physical NIC.        |
| 2     | Kernel's routing sends packet to TUN.    | Kernel's network stack processes packet.|
| 3     | Packet forwarded to TUN device.          | Packet may be filtered, NAT'd, etc.    |
| 4     | User-space app reads packet from TUN.    | OS passes packet to appropriate socket |
| 5     | User-space stack processes packet.       | Application reads packet from socket.  |
| 6     | Optional: User-space modifies packet.    | N/A                                    |
| 7     | Optional: Packet sent out via TUN.       | N/A                                    |
| 8     | Kernel routes the outgoing packet.       | Kernel routes the outgoing packet.     |

- **With a TUN Device**: The kernel does minimal work. It forwards the packet to the TUN device, allowing a user-space application (our TCP application) to handle most of the packet processing, including optional modifications and potential retransmission.
    
- **Without a TUN Device**: The kernel's own network stack handles the packet completely, including any routing, filtering, and NAT operations. The application simply reads the packet from a socket, abstracted from the underlying details.

Now that we have a good understanding of why we need to use a TUN device we can get started with writing some code. 
You can create a brand new Rust project by running 
```
cargo new blah blaj
```

We will be using this TunTap crate which is a rust wrapper around the Tun/Tap drivers. To add it to our project, we simply add the following line to our Cargo.Toml file. 
```
tun-tap = "0.1.4"
```

```
use std::io;

fn main() -> io::Result<()> {
    // Create a new TUN interface named "tun0" in TUN mode.
    let nic = tun_tap::Iface::new("tun0", tun_tap::Mode::Tun)?;

    // Define a buffer of size 1504 bytes (maximum Ethernet frame size without CRC) to store received data.
    let mut buf = [0u8; 1504];

    // Main loop to continuously receive data from the interface.
    loop {
        // Receive data from the TUN interface and store the number of bytes received in `nbytes`.
        let nbytes = nic.recv(&mut buf[..])?;

		eprintln!("read {} bytes: {:x?}", nbytes, &buf[..nbytes]);
    }

    Ok(())
}

```

After building the binary using `cargo b --release`, we need to elevate the permissions of the compiled binary. We achieve this by running `sudo setcap cap_net_admin=eip ./target/release/tcp`. This grants the binary the required privileges for manipulating network interfaces and routing tables.

Once we've executed our binary, a new virtual network interface called "tun0" is created. To verify its existence, we can run `ip addr`, which should display a list of all network interfaces on our machine including the newly created "tun0" which is generally at the bottom of the list as seen below.
```
4: tun0: <POINTOPOINT,MULTICAST,NOARP> mtu 1500                               qdisc noop state DOWN group default qlen 500
link/none 
```

But notice, it doesn't have an IP address at this point. So we can't  exactly send any packets to it. To remedy that, we'll execute `sudo ip addr add 192.168.0.1/24 dev tun0` which will assign the IP address `192.168.0.1` with a subnet mask of `255.255.255.0` (represented by `/24`) to the network interface we created named `tun0`. 
we can run the `ip addr` command again to confirm, You will see an output like below which confirms our virtual network interface now has an IP address attached which we can ping and send packets to.
```
4: tun0: <POINTOPOINT,MULTICAST,NOARP> mtu 1500 
qdisc noop state DOWN group default qlen 500
link/none    
inet 192.168.0.1/24 scope global tun0
valid_lft forever preferred_lft forever
```

Next, we activate the network interface by executing `sudo ip link set up dev tun0`.

Now we're all set to test. If you recall,  earlier we said we will be dealing with raw network packets in out user space program that the kernel sends us, well lets see it in action. Go ahead and run the command below to ping our virtual network interface or any subnet within it [while still executing our binary].

```
ping -I tun0 192.168.0.2 
```

You will notice that our application receives some raw bytes of date. Something like below. That is amazing.
```
[0, 0, 86, dd, 60, 0, 0, 0, 0, 8, 3a, ff, fe, 80, 0, 0, 0, 0, 0, 0, 15, 62, d0, a2, 5c, 4e, c2, 45, ff, 2, 0, 0, 0, 0, 0, 0,0, 0, 0, 0, 0, 0, 0, 2, 85, 0, 78, 9e, 0, 0, 0, 0]]
```


**Aside** 

For efficiency, we can script the entire process:
```
#!/bin/bash
cargo b --release
sudo setcap cap_net_admin=eip ./target/release/tcp
./target/release/tcp & 
pid=$1
sudo ip addr add 192.168.0.1/24 dev tun0
trap "kill $pid" INT TERM
wait $pid

```

### References
- [Virtual networking 101: Bridging the gap to understanding TAP](https://blog.cloudflare.com/virtual-networking-101-understanding-tap/)
- [Corresponding Code](https://github.com/jonhoo/rust-tcp/commit/b7c28eecf7c7f20a38a1e0d48f91fc2b703b0d47#diff-42cb6807ad74b3e201c5a7ca98b911c5fa08380e942be6e4ac5807f8377f87fc)
