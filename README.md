# rust-dns

Simple and super useful DNS over HTTPS proxy server.  Mostly an exercise to learn more async/await in Rust, but stable enough that I'm using this as the only DNS server on my home network.

In short this app listens on normal/legacy DNS UDP and TCP sockets on the local network.  It proxies to an upstream DNS over HTTPS server and caches the results.  Also supports simple forward and reverse host/IP mappings to allow authorative lookups on a local domain.

Tech Stack:
* [tokio](https://crates.io/crates/tokio) Asnyc I/O runtime for rust.  Using this directly to do async file I/O, TCP and UDP sockets, timers, and timeouts.
* [trust-dns-proto](https://crates.io/crates/trust-dns-proto) a nice library for marshalling and umarshalling binary DNS messages to Rust DTOs.  Ignoring the warning that this library should not be used directly. :)
* [RFC8484 DNS over HTTPS](https://tools.ietf.org/html/rfc8484) protocol for upstream requests.
* [reqwest](https://crates.io/crates/reqwest) HTTP client.  This does HTTP2, is based on hyper (which is based on tokio), and supports async/await.
* [lru](https://crates.io/crates/lru) LRU cache.

## Configuration
See config directory for examples.

## Systemd
See systemd directory for an example user unit file.

## Cross compile
Using [cross](https://github.com/rust-embedded/cross) to compile for x86_64 Linux on MacOS:

```cross build --target x86_64-unknown-linux-gnu --release```
