## Parsing The Bits
Now that we have our virtual network interface all set up and it receives bits of data the next step in our journey of implementing TCP is to parse out the packets from the bytes of data we receive. By default, alongside the packets sent and received from the virtual network interface, an additional 4 bytes of data is prepended to the packet. The [Tun/TAP documentation section 3.2](https://www.kernel.org/doc/Documentation/networking/tuntap.txt) informs us of the structure of the data we receive. The first 2 bytes are the flags that can give us more information about the packet we received, for example, the "TUN_PKT_STRIP" which is set by the kernel to signal the userspace program that the packet was truncated because the buffer was too small. The next two bytes is the proto field which specifies the IP version of the protocol. After the first 4 meta bytes, the rest of the data is the Raw protocol. 
So we shall modify our code to add the flag and proto bytes. To do that we use the `u16::from_be_bytes` method to read the first 2 bytes of the packet and get us a human-readable value of the flag. Remember network order is big endian hence us using the from big endian bytes method.
Replace the content of your loop with the following
```rust
loop { 
	let nbytes = nic.recv(&mut buf[..])?;
	let flags = u16::from_be_bytes([buf[0], buf[1]]); 
	let proto = u16::from_be_bytes([buf[2], buf[3]]); 
	eprintln!("read {} bytes: {:x?}", nbytes - 4, &buf[4..nbytes]);   
    }     
```

Now when we run our program, we can notice the flags and proto fields being printed out. In my case, the flag prints a value of 0 & proto prints out "86dd". To make sense of what the proto field means we can look at this table mapping ether types to protocols and we can see that the value of the proto field we parsed corresponds to [Internet Protocol Version 6](https://en.wikipedia.org/wiki/Internet_Protocol_Version_6 "Internet Protocol Version 6") (IPv6). 

Since we are going to be focusing on ipv4 in this implementation we can add a conditional check to our code to ignore any packets whose ether type is not ipv4 by adding this line

```rust
if proto != 0x0800{
//This is Not IPV4 skip it.
continue;
}
```

Now that we have made sense of the first 4 bytes which were prepended to the ethernet frame by the kernel, we are left with the main TCP data and we need to make sense of it. Since our main concern is implementing the protocol, we shall once again recruit a crate to help parse the IP packet and TCP information. Although we will be using a crate to do the parsing, It is important to still understand what is happening. We are essentially going to be encoding the IPV4 header in our code 
<img width="951" alt="Screenshot 2023-10-31 at 16 49 50" src="https://github.com/Ghvstcode/Rust-Tcp/assets/46195831/8eaf421d-603e-4c19-a033-20020085f2c5">

Once we decode the IP packet we will be able to get some important information like the Destination Address, The source address, and the protocol. 
The crate we shall be using for parsing the IP Packet header is etherparse. To add it to your project add the the following to your cargo.toml
```
etherparse = "0.13.0"
```

```rust
// Attempt to parse an IPv4 header from the provided buffer slice.
match etherparse::Ipv4HeaderSlice::from_slice(&buf[4..nbytes]) {
    // If the parsing was successful, proceed with the parsed packet.
    Ok(iph) => {
        // Extract the source IP address from the parsed packet.
        let src = iph.source_addr();
        
        // Extract the destination IP address from the parsed packet.
        let dst = iph.destination_addr();
        
        // Extract the protocol number from the parsed packet.
        // For TCP, this number is typically 6 (0x06).
        let proto = iph.protocol();

        // Check if the protocol number is not TCP (0x06).
        if proto != 0x06 {
            // If the packet is not a TCP packet, skip further processing.
            continue;
        }

        // Attempt to parse the TCP header from the buffer slice.
        // Here, we adjust the starting slice based on the length of the IPv4 header.
        match etherparse::TcpHeaderSlice::from_slice(&buf[4 + p.slice().len()..]) {
            // If TCP header parsing was successful, proceed.
            Ok(tcph) => {
                // Print the details: Source IP, Destination IP, and the Destination Port.
                eprintln!("{} -> {}: TCP to port {}", src, dst, tcph.destination_port());
            }
            // Handle potential errors while parsing the TCP header.
            Err(e) => {
                eprintln!("An error occurred while parsing TCP packet: {:?}", e);
            }
        }
    }
    // Handle potential errors while parsing the IPv4 header.
    Err(e) => {
        eprintln!("An error occurred while parsing IP packet: {:?}", e);
    }
}

```
On running the code above & using a TCP client to ping our application, `nc 192.168.0.2 80`  you should see the TCP packets being received alongside the destination address, the source address, and the protocol.
### References
- [Corresponding Code](https://github.com/jonhoo/rust-tcp/commit/b7c28eecf7c7f20a38a1e0d48f91fc2b703b0d47#diff-42cb6807ad74b3e201c5a7ca98b911c5fa08380e942be6e4ac5807f8377f87fc)
