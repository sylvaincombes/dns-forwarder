# dns-forwarder

A minimal, static UDP DNS proxy for [Socktainer](https://github.com/socktainer/socktainer) — the Docker-compatible API shim for [Apple Container](https://github.com/apple/container) on macOS Apple Silicon.

Written in Rust with zero external runtime dependencies, target is to run inside an Apple Container Linux vm (arm64).

## Context

Socktainer runs Linux containers inside Apple Container's lightweight VMs on macOS Apple Silicon (arm64). Socktainer creates one `dns-forwarder` sidecar per network it manages (`NetworkDNSManager`, keyed by network id). Containers send DNS queries to their network's forwarder, which relays them to Socktainer's `SocktainerDNSServer` running on the host macOS gateway. This gives every compose service transparent hostname resolution for its peers (e.g. `web` resolving `db`).

Apple Container currently exposes a single shared network for all containers, so in practice this is **one forwarder serving every container across all compose projects** on the host. The per-network design keeps the forwarder reusable, but the sizing below is tuned for that host-wide reality.

```
macOS host
├── SocktainerDNSServer :2054   ← resolves container names
└── Apple Container VMs (per compose network)
    ├── dns-forwarder :53       ← this binary
    ├── container A             → DNS queries → dns-forwarder → SocktainerDNSServer
    └── container B             → DNS queries → dns-forwarder → SocktainerDNSServer
```

The forwarder is configured at container start via a single environment variable:

| Variable | Example | Description |
|---|---|---|
| `DNS_UPSTREAM` | `192.168.64.1:2054` | Gateway IP of the compose network + port of `SocktainerDNSServer` on the host |

## Target platform

| Property | Value |
|---|---|
| CPU | ARM64 (Apple Silicon — M1/M2/M3/M4) |
| Runs inside | Linux VM (Apple Container, arm64) |
| Binary format | ELF 64-bit ARM aarch64, statically linked (musl libc) |
| Built from | macOS via Docker or Podman (`make build`), or natively on Linux arm64 (CI) |

The binary is `FROM scratch` — no base OS, no shell, no package manager. It runs directly as PID 1 in its container.

## Design

The implementation is intentionally minimal (~80 lines of Rust, zero external runtime dependencies). Each incoming UDP query spawns a dedicated thread with a 32 KB stack. Sizing is tuned for the actual traffic profile: DNS queries are short internal A-record lookups (40–150 bytes), with ~15 concurrent at a single project's startup. Since one forwarder currently serves every project on Apple Container's shared network, `MAX_CONCURRENT` (64) is sized for the host-wide burst rather than a single project.

```
recv_from(:53)
    │
    ├── inflight ≥ 64 ──► drop (DNS clients retry automatically)
    │
    └── thread (32 KB stack)
            ├── connect UDP to DNS_UPSTREAM
            ├── send(query)
            ├── recv(response)  ← 1 s timeout
            └── send_to(client)
```

`tokio` is deliberately not used. For up to 64 max concurrent queries over the Apple Container virtual network (effectively loopback), `std::thread` + `std::net::UdpSocket` are simpler, produce a significantly smaller binary, and require no async runtime.

### Sizing rationale

| Constant | Value | Rationale |
|---|---|---|
| `MAX_DNS_SIZE` | 1232 bytes | RFC 8085 safe UDP ceiling; compose queries are 40–150 bytes in practice |
| `MAX_CONCURRENT` | 64 | One forwarder serves all containers on Apple Container's single shared network (a heavy user may run 50–100 across projects); 64 covers that host-wide burst. Worst-case 64 × 32 KB = 2 MB stack |
| `UPSTREAM_TIMEOUT` | 1 s | Upstream is SocktainerDNSServer on the macOS host (≈ loopback); 1 s releases stalled threads quickly, 2× faster than the original 2 s |
| `THREAD_STACK` | 32 KB | `forward()` uses < 4 KB; 8× headroom, 256× smaller than Rust's default |

## Performance

Measured on Apple Silicon M5 via loopback benchmarks (`cargo bench`, macOS native):

| Metric | Value |
|---|---|
| Latency (loopback) | ~91 µs per round-trip |
| Throughput (loopback) | ~11,000 queries/sec |
| Linux arm64 musl binary | 388 KB |
| OCI image size | 224 KB |
| OCI tarball (gzipped) | **225 KB** |
| RAM at runtime (typical) | ~1–2 MB |
| RAM at runtime (worst case, 64 concurrent) | ~3 MB |

For reference: CoreDNS (previously used by Socktainer) consumed 21 MB on DockerHub pull and 76 MB RAM per compose network.

## Development — macOS Apple Silicon

This is the primary development platform. No musl toolchain is needed on the host — compilation for the Linux target happens inside the container engine (Docker by default; Podman or Apple `container` via `ENGINE=`).

### Requirements

- macOS on Apple Silicon (M1 or later)
- [Rust](https://rustup.rs) stable — `rust-toolchain.toml` pins the version; `rustup` installs it automatically on first `cargo` invocation
- A container engine — only needed for `make build`/`make tarball`: [Docker Desktop](https://www.docker.com/products/docker-desktop/), [Colima](https://github.com/abiosoft/colima), Podman, or Apple `container` (select with `ENGINE=`, see below)

### Quick start

```sh
# Clone and enter the repo
git clone https://github.com/socktainer/dns-forwarder
cd dns-forwarder

# Run tests (native macOS — no Docker needed)
make test

# Run benchmarks (native macOS — no Docker needed)
make bench

# Build the Linux arm64 OCI image (requires a container engine)
make build

# Export as gzipped OCI tarball
make tarball

# Use a different engine (default is docker)
make build ENGINE=podman
make tarball ENGINE=container
```

### What runs natively vs in Docker

| Command | Runs on | Engine needed |
|---|---|---|
| `make test` / `cargo test` | macOS (native) | No |
| `make bench` / `cargo bench` | macOS (native) | No |
| `make build` | Linux arm64 inside the engine | **Yes** (docker/podman/container) |
| `make tarball` | Linux arm64 inside the engine | **Yes** (docker/podman/container) |

`cargo build` without an explicit `--target` compiles a native macOS binary — useful for IDE tooling and fast iteration, but the macOS binary cannot run inside Apple Container VMs.

The `.cargo/config.toml` sets `musl-gcc` as the linker for `aarch64-unknown-linux-musl`. This config is **only active on Linux** (CI). On macOS, it is silently ignored for any build that does not explicitly pass `--target aarch64-unknown-linux-musl` — which you should not do on macOS directly; use `make build` instead.

## Testing

```sh
make test         # equivalent to cargo test
make bench        # criterion benchmark: loopback latency and throughput
```

### Test coverage

| Test | What it verifies |
|---|---|
| `test_forward_relays_query_and_response` | End-to-end: query reaches mock upstream, transformed response delivered to correct client address |
| `test_forward_timeout_is_silent` | Unresponsive upstream: 1s timeout fires, no panic, no response to client |
| `test_concurrency_limit_boundary` | Atomic semaphore: MAX\_CONCURRENT-th query accepted, (MAX+1)-th dropped, slot released correctly |

## CI

Two workflows run on **GitHub Actions arm64 runners** (`ubuntu-24.04-arm`) — no QEMU emulation, no Docker daemon required.

| Workflow | Trigger | What it does |
|---|---|---|
| [`ci.yml`](.github/workflows/ci.yml) | Push / PR | `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, musl release build |
| [`release.yml`](.github/workflows/release.yml) | Manual dispatch (version input) | Compiles the static binary, builds the OCI image with `buildah`, publishes the SwiftPM package + image to the `swiftpm` branch, tags `vX.Y.Z`, and attaches `socktainer-dns.tar.gz` to a GitHub Release |

CI uses `buildah` (daemonless, explicitly installed in each job) rather than `docker build`. This avoids needing a Docker daemon or privileged container, and compiles natively on arm64 Linux rather than under QEMU emulation.

> **Note:** The CI workflows are wired but unverified until the first push/tag. The `musl-tools` and `buildah` packages are explicitly installed in each job rather than assumed pre-installed.

## Integration with Socktainer

The forwarder is published as a SwiftPM package on the `swiftpm` branch, tagged per release. Socktainer consumes it like any Swift dependency — `swift build` resolves and fetches it, with no manual fetch step:

```swift
// socktainer/socktainer Package.swift
.package(url: "https://github.com/socktainer/dns-forwarder.git", exact: "0.1.0"),
// target dependency:
.product(name: "SocktainerDNSImage", package: "dns-forwarder"),
```

The package carries the OCI image as a SwiftPM resource, loaded via `Bundle.module` and imported into Apple Container's image store on first use — no network access at runtime. SwiftPM can't reference a remote OCI tarball by URL (`.binaryTarget` only accepts an artifactbundle/xcframework), so the image is carried as a resource committed on the `swiftpm` branch; `main` stays pure Rust with no blob.

To bump the forwarder, change the `exact:` version (or `swift package update` with a range); the resolved commit is pinned in `Package.resolved`.

The same release also attaches `socktainer-dns.tar.gz` as a GitHub Release asset, for any non-SwiftPM consumer that prefers a direct download.

## Project structure

```
.cargo/config.toml          # musl-gcc linker (Linux only; ignored on macOS)
.github/
  workflows/
    ci.yml                  # lint + test on every push/PR (ubuntu-24.04-arm)
    release.yml             # dispatch: build, publish swiftpm branch, tag, release
benches/
  forward.rs                # criterion benchmark: loopback round-trip
packaging/
  swift/                    # SwiftPM package, copied to the `swiftpm` branch on release
    Package.swift           #   thin manifest exposing the OCI image as a resource
    Sources/SocktainerDNSImage/SocktainerDNSImage.swift
src/
  lib.rs                    # forward() + unit tests (pub for benchmarks)
  main.rs                   # UDP receive loop + thread dispatch
Cargo.toml                  # no runtime deps; criterion in dev-deps only
Dockerfile                  # rust:alpine (AWS ECR Public mirror) → FROM scratch; local builds
Makefile                    # make test / bench / build / tarball (ENGINE=docker|podman|container)
rust-toolchain.toml         # pins Rust stable + aarch64-unknown-linux-musl target
```

## License

Apache 2.0 — see [LICENSE](LICENSE).
