## Implementing TCP  In Rust
Welcome to Implementing TCP in Rust. This is a book with blogpost sized chapters walking you through implementing TCP in Rust. I've been into Rust for a bit, read some books, but nothing beats a real project to make it all click. I was on the lookout for a solid project to sink my teeth into and really get Rust into my fingers. I discovered [Jon Gjengset's](https://www.youtube.com/@jonhoo) epic live stream of building a TCP stack in Rust. Watching him work through the process was a game-changer for me, and honestly, this book wouldn't have happened without his streams as a starting point.

At first, this book was just a set of notes for me to refer back to. But then it hit me – why not flesh these notes out into something more substantial? Jon's streams are gold - they're incredibly in-depth and he explains his every move and thought for hours(The three videos on TCP were for about 14 hours). But I get it, not everyone can or wants to commit to that length of video content.

So here's what I've got: a book inspired by Jon's marathon sessions, condensed into a compact format for the rest of us. It's for anyone who prefers reading and doing over watching.

### What Are We Building?
In this book, we embark on a  journey to build a user space TCP stack in Rust, grounded in the specifications of [RFC 793](https://www.rfc-editor.org/rfc/rfc793). The TCP stack is an implementation that lives entirely in user space, as opposed to being part of the operating system kernel. This choice offers us an opportunity to understand and tinker with network programming without delving into kernel development.

Transmission Control Protocol (TCP) is a fundamental protocol that forms the backbone of the internet. It is what allows us to send and receive data with the assurance of reliability and order. Without TCP, the internet as we know it—complete with the seamless sending of emails, files, and the browsing of websites—would be untenable. TCP guarantees that packets of data arrive at their destination correctly and in the same order they were sent, providing essential services such as data sequencing, error detection, and correction through retransmissions.

I encourage you to read [RFC 793](https://www.rfc-editor.org/rfc/rfc793). It is very straightforward and most of what we will be discussing in the coming chapters is based off it.

### How To Read This book
This is the order in which to read this book: 
* [Chapter 1 - The Kernel-User Space Divide](https://github.com/Ghvstcode/Rust-Tcp/blob/main/chapter1/chapter1.md#the-kernel-user-space-divide)
* [Chapter 2 - Parsing The Bits](https://github.com/Ghvstcode/Rust-Tcp/blob/main/chapter2/chapter2.md#parsing-the-bits)
* [Chapter 3 - The connection Quad](https://github.com/Ghvstcode/Rust-Tcp/blob/main/chapter3/chapter3.md#the-connection-quad)
* [Chapter 4 - The Final Handshake](https://github.com/Ghvstcode/Rust-Tcp/blob/main/chapter4/chapter4.md#the-final-handshake)
* Chapter 5(coming soon)

I have deliberately not included complete code samples within the book, to nudge you towards active learning—writing the code yourself rather than copying it. But don’t worry, each chapter is paired with a relevant commit link to Jon's repository, where you can check your work. If I get [requests](https://github.com/Ghvstcode/Rust-Tcp/issues/new) to include code samples to each chapter then I shall consider doing so.

By the end of this book, you will have a working TCP stack that you can use to understand and explore the lower levels of network programming. You will gain practical knowledge in Rust and a deeper appreciation for the protocols that power the internet. This book will serve not only as a guide to implementing RFC 793 but also as a testament to the power and capabilities of Rust in systems programming.

### Contributing To This Book

I welcome any and all contributions to make 'Implementing TCP in Rust' a better resource for everyone who comes after you. If you spot an error, have a suggestion, or want to contribute in any other way, please don't hesitate to reach out. Your feedback is invaluable, whether it's a typo, a technical clarification, or a new perspective on how to tackle a problem.

I do not claim to be the ultimate authority on Rust or TCP implementation; I'm learning right along with you. This book is a shared journey, and every reader's experiences can help improve it for future readers. So if you have a correction, a better way to explain a concept, or any other input, I'm all ears.

### Here's How You Can Contribute:

- **Suggestions and Corrections:** If you have suggestions for improvements or find any mistakes, please feel free to submit them as an issue or pull request to the repository.
    
- **Sharing Knowledge:** If you have insights or additional information that could benefit other readers, consider contributing a section or a note.
    
- **Expanding Content:** If you've discovered a new method or have a request for additional content, let me know, and we can expand the book's coverage together.

If you encounter an issue or have suggestions, here's a suggested workflow to intoduce changes:

1. **Open an Issue:** Found a bug or a confusing section? Have a suggestion? Please start by opening an issue on the [book's GitHub repository](https://github.com/Ghvstcode/Rust-Tcp). This is a great way to discuss potential changes and allow for a public conversation around your feedback.
    
2. **Submit a Pull Request (PR):** Once we've discussed the issue and agreed on a course of action, you can fork the repository, make the proposed changes, and submit a PR. It's a good practice to reference the original issue in your PR to help keep track of the discussion and rationale behind your contribution.
    
3. **Discussion:** Some issues may spark further discussion and lead to additional insights or changes. I encourage this dialogue as it enhances the material and supports a vibrant learning community.

Together, we can ensure that 'Implementing TCP in Rust' remains a current and useful resource for learning and applying Rust to real-world problems. Thank you for your interest in contributing to this book, and I look forward to your input.

### Support

The creation of 'Implementing TCP in Rust' has been a labor of love, reflecting my passion for open-source learning and knowledge sharing. If you appreciate this work and are in a position to support further development, here are a few ways you can help:

- **Job Opportunities**: I am actively seeking new opportunities and am open to full-time roles that align with my skills in deep technical writing, developer advocacy, or software engineering(Golang and Rust) or technical product management. If you know of such roles or have leads, please reach out via email => me@ghvsted.com 
- **Star and Share**: If you find this resource useful, please star the repo and share it with your network. 
- **Collaboration and Networking**: I'm always looking to collaborate on exciting projects and make new connections in the tech community. If you're interested in collaborating or just want to network, feel free to connect with me. <br>


Your support is not only about contributing code or ideas; it's also about opening doors to new possibilities and endeavors. Thank you for any leads, opportunities, and collaborations you provide.

### License

This book, "Implementing TCP in Rust," is licensed under the Creative Commons Attribution 4.0 International License (CC BY 4.0). This permits use, sharing, adaptation, distribution, and reproduction in any medium or format, as long as appropriate credit is given.

### References And Additional Resources
* [Implementing TCP in Rust - Jons Stream](https://youtu.be/bzja9fQWzdA?si=DMnNMudTbzwgmlkz)
* [RFC 793](https://www.rfc-editor.org/rfc/rfc793)
* [TapIP Network Topology](https://github.com/chobits/tapip/blob/master/doc/net_topology)
* [Sami's TCP/IP stack written in C](https://github.com/saminiir/level-ip)
* ChatGPT

