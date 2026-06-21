use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use dns_forwarder::forward;
use std::net::UdpSocket;
use std::time::Duration;

/// Measures end-to-end latency of forward() over loopback (no real DNS).
/// Reports both latency (µs) and throughput (queries/sec).
fn bench_forward_loopback(c: &mut Criterion) {
    // Upstream that immediately echoes back.
    let upstream = UdpSocket::bind("127.0.0.1:0").unwrap();
    let upstream_addr = upstream.local_addr().unwrap();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 4096];
        loop {
            if let Ok((n, peer)) = upstream.recv_from(&mut buf) {
                let _ = upstream.send_to(&buf[..n], peer);
            }
        }
    });

    let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let client_addr = client.local_addr().unwrap();

    let mut drain_buf = vec![0u8; 4096];

    // Minimal valid-ish DNS query bytes.
    let query: &[u8] = b"\x00\x01\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00";

    let mut group = c.benchmark_group("forward");
    group.throughput(Throughput::Elements(1));
    group.bench_function("loopback_roundtrip", |b| {
        b.iter(|| {
            forward(&listener, client_addr, query, upstream_addr);
            client
                .recv(&mut drain_buf)
                .expect("benchmark iteration did not receive a forwarded response");
        })
    });
    group.finish();
}

criterion_group!(benches, bench_forward_loopback);
criterion_main!(benches);
