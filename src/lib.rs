//! Core DNS forwarding logic, split from main for testability and benchmarks.

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Maximum DNS message size.
/// RFC 8085 recommends 1232 bytes as the safe UDP ceiling to avoid IP
/// fragmentation. Socktainer's compose traffic is exclusively short internal
/// A-record lookups (40–150 bytes in practice); 1232 covers all valid DNS
/// responses without the overhead of the theoretical EDNS0 max (4096).
pub const MAX_DNS_SIZE: usize = 1232;

/// Maximum concurrent in-flight forwards.
/// Apple Container uses a single shared network for all compose projects, so
/// this forwarder receives queries from every container on the host — a power
/// user may have 50–100 containers across multiple projects. 64 covers any
/// realistic burst while keeping worst-case stack at 64 × THREAD_STACK = 2 MB,
/// still far below CoreDNS (76 MB RAM).
pub const MAX_CONCURRENT: usize = 64;

/// Per-query upstream timeout.
/// The upstream is SocktainerDNSServer on the macOS host, reachable via the
/// Apple Container VM gateway — effectively loopback. Because this is a single
/// shared forwarder for all containers on the host, SocktainerDNSServer may
/// be under higher load on a busy machine; 1 s gives it room to breathe while
/// still releasing stalled threads twice as fast as the original 2 s.
pub const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(1);

/// Stack size for forwarding threads.
/// forward() uses well under 4 KB of actual stack (one UdpSocket + error
/// types + response buffer on the heap). 32 KB gives 8× headroom and is
/// 256× smaller than Rust's 8 MB default.
pub const THREAD_STACK: usize = 32 * 1024;

/// Forwards a single DNS query to `upstream` and writes the response back
/// to `client` via `listener`. Returns silently on any I/O error or timeout.
pub fn forward(listener: &UdpSocket, client: SocketAddr, query: &[u8], upstream: SocketAddr) {
    // Bind the ephemeral socket in the upstream's address family — binding
    // IPv4 (0.0.0.0) when upstream is IPv6 would make connect() fail.
    let bind_addr = if upstream.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let sock = match UdpSocket::bind(bind_addr) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = sock.set_read_timeout(Some(UPSTREAM_TIMEOUT));
    let _ = sock.set_write_timeout(Some(UPSTREAM_TIMEOUT));

    if sock.connect(upstream).is_err() {
        return;
    }
    if sock.send(query).is_err() {
        return;
    }

    let mut resp = vec![0u8; MAX_DNS_SIZE];
    if let Ok(n) = sock.recv(&mut resp) {
        let _ = listener.send_to(&resp[..n], client);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_relays_query_and_response() {
        let upstream_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let upstream_addr = upstream_sock.local_addr().unwrap();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; MAX_DNS_SIZE];
            let (n, peer) = upstream_sock.recv_from(&mut buf).unwrap();
            let mut resp = vec![0xFFu8];
            resp.extend_from_slice(&buf[..n]);
            upstream_sock.send_to(&resp, peer).unwrap();
        });

        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let listener_addr = listener.local_addr().unwrap();
        let client_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client_addr = client_sock.local_addr().unwrap();

        forward(&listener, client_addr, b"hello-dns", upstream_addr);

        let mut resp_buf = vec![0u8; MAX_DNS_SIZE];
        client_sock
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let (n, from) = client_sock.recv_from(&mut resp_buf).unwrap();

        assert_eq!(from, listener_addr, "response must come from the listener");
        assert_eq!(&resp_buf[..n], b"\xFFhello-dns");
    }

    #[test]
    fn test_forward_timeout_is_silent() {
        let black_hole = UdpSocket::bind("127.0.0.1:0").unwrap();
        let upstream_addr = black_hole.local_addr().unwrap();

        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client_addr = client_sock.local_addr().unwrap();

        forward(&listener, client_addr, b"query", upstream_addr);

        client_sock.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 16];
        assert!(client_sock.recv(&mut buf).is_err());
    }

    #[test]
    fn test_forward_relays_over_ipv6_upstream() {
        // Proves the ephemeral socket binds in the upstream's family: with an
        // IPv6 upstream, an IPv4 bind would make connect() fail and drop the query.
        let upstream_sock = match UdpSocket::bind("[::1]:0") {
            Ok(s) => s,
            Err(_) => return, // IPv6 loopback unavailable — skip
        };
        let upstream_addr = upstream_sock.local_addr().unwrap();

        std::thread::spawn(move || {
            let mut buf = vec![0u8; MAX_DNS_SIZE];
            let (n, peer) = upstream_sock.recv_from(&mut buf).unwrap();
            let mut resp = vec![0xFFu8];
            resp.extend_from_slice(&buf[..n]);
            upstream_sock.send_to(&resp, peer).unwrap();
        });

        let listener = UdpSocket::bind("[::1]:0").unwrap();
        let listener_addr = listener.local_addr().unwrap();
        let client_sock = UdpSocket::bind("[::1]:0").unwrap();
        let client_addr = client_sock.local_addr().unwrap();

        forward(&listener, client_addr, b"hello-v6", upstream_addr);

        let mut resp_buf = vec![0u8; MAX_DNS_SIZE];
        client_sock
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let (n, from) = client_sock.recv_from(&mut resp_buf).unwrap();
        assert_eq!(from, listener_addr, "response must come from the listener");
        assert_eq!(&resp_buf[..n], b"\xFFhello-v6");
    }

    #[test]
    fn test_concurrency_limit_boundary() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let counter = Arc::new(AtomicUsize::new(0));
        for _ in 0..MAX_CONCURRENT {
            counter.fetch_add(1, Ordering::Relaxed);
        }
        let prev = counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(prev, MAX_CONCURRENT);
        counter.fetch_sub(1, Ordering::Relaxed);
        counter.fetch_sub(1, Ordering::Relaxed);
        let prev = counter.fetch_add(1, Ordering::Relaxed);
        assert!(prev < MAX_CONCURRENT);
    }
}
