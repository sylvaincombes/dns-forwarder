use dns_forwarder::{MAX_CONCURRENT, MAX_DNS_SIZE, THREAD_STACK, forward};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn main() {
    let upstream_var = std::env::var("DNS_UPSTREAM")
        .expect("DNS_UPSTREAM env var must be set (e.g. 192.168.64.1:2054)");
    // Resolve the upstream once at startup. Resolving per request would block
    // the worker thread and is not bounded by UPSTREAM_TIMEOUT (socket timeouts
    // cover only send/recv, not address resolution).
    let upstream: SocketAddr = upstream_var
        .to_socket_addrs()
        .unwrap_or_else(|e| panic!("DNS_UPSTREAM '{upstream_var}' is not resolvable: {e}"))
        .next()
        .unwrap_or_else(|| panic!("DNS_UPSTREAM '{upstream_var}' resolved to no addresses"));

    let listener = Arc::new(UdpSocket::bind("0.0.0.0:53").expect("failed to bind :53"));
    eprintln!("dns-forwarder: listening on :53, upstream {upstream}");

    let inflight = Arc::new(AtomicUsize::new(0));
    let mut buf = vec![0u8; MAX_DNS_SIZE];

    loop {
        let (n, client_addr) = match listener.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Drop query if at capacity — DNS clients retry automatically.
        if inflight.fetch_add(1, Ordering::Relaxed) >= MAX_CONCURRENT {
            inflight.fetch_sub(1, Ordering::Relaxed);
            continue;
        }

        let query = buf[..n].to_vec();
        let sock = listener.clone();
        let up = upstream;
        let ctr = inflight.clone();

        if std::thread::Builder::new()
            .stack_size(THREAD_STACK)
            .spawn(move || {
                forward(&sock, client_addr, &query, up);
                ctr.fetch_sub(1, Ordering::Relaxed);
            })
            .is_err()
        {
            // spawn failed (resource exhaustion) — undo the inflight increment
            // so the counter doesn't ratchet toward MAX_CONCURRENT permanently.
            inflight.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
