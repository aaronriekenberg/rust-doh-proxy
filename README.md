# rust-doh-proxy

Simple and super useful DNS over HTTPS proxy server.  Mostly an exercise to learn more async/await in Rust, but stable enough that I'm using this as the only DNS server on my home network.

In short this app listens on normal/legacy DNS UDP and TCP sockets on the local network.  It proxies to an upstream DNS over HTTPS server and caches the results.  Also supports simple forward and reverse host/IP mappings to allow authorative lookups on a local domain.

Tech Stack:
* [tokio](https://crates.io/crates/tokio) Asnyc I/O runtime for rust.  Using this directly to do async file I/O, TCP and UDP sockets, timers, and timeouts.
* [trust-dns-proto](https://crates.io/crates/trust-dns-proto) a nice library for marshalling and umarshalling binary DNS messages to Rust DTOs.  Ignoring the warning that this library should not be used directly. :)
* [RFC8484 DNS over HTTPS](https://tools.ietf.org/html/rfc8484) protocol for upstream requests.
* [reqwest](https://crates.io/crates/reqwest) HTTP client.  This does HTTP2, is based on hyper (which is based on tokio), and supports async/await.
* [lru](https://crates.io/crates/lru) LRU cache.

## How do I run this?
After building with cargo, you can run the app as follow.  Since this is using [env_logger](https://crates.io/crates/env_logger) need to set RUST_LOG variable to get log output:

```
RUST_LOG=info ./target/debug/rust-doh-proxy ./config/config.json
```

If all is well you will see these logs that the app is listening on 127.0.0.1:10053.

```
[INFO  rust_doh_proxy::doh::udpserver] listening on udp 127.0.0.1:10053
[INFO  rust_doh_proxy::doh::tcpserver] listening on tcp 127.0.0.1:10053
```

Then you can use [dig](https://en.wikipedia.org/wiki/Dig_(command)) for example to make a DNS query to the app:
```
dig -p 10053 @127.0.0.1 google.com
```

Normally DNS uses a privileged port 53.  In this example this app is using unprivileged port 10053 to run as a normal user.  The listen address and port are configurable in the configuration json file.

To actually use this app as a server and accept connections on port 53 I use nftables on linux with a redirect rule to redirect incoming requests on port 53 to port 10053.


## Configuration
See config directory for examples.

## Systemd
See systemd directory for an example user unit file.

## Cross compile
Using [cross](https://github.com/rust-embedded/cross) to compile for x86_64 Linux on MacOS:

```cross build --target x86_64-unknown-linux-gnu --release```
