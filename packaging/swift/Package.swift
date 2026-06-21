// swift-tools-version: 6.0
//
// SwiftPM distribution package for the socktainer-dns OCI image.
//
// This manifest does NOT live at the root of `main` — `main` is the Rust source.
// On each release, the workflow copies these files to the root of the `swiftpm`
// branch, drops the freshly built `socktainer-dns.tar.gz` next to the source, and
// tags that commit `vX.Y.Z`. socktainer then consumes a specific version with:
//
//     .package(url: "https://github.com/socktainer/dns-forwarder.git", exact: "X.Y.Z")
//
// SwiftPM resolves by tag (branch-agnostic), checks out that commit, and bundles
// the tarball as a resource. There is no SwiftPM way to reference the tarball by
// URL (`.binaryTarget` only accepts artifactbundle/xcframework), so the package
// carries the image as a `.copy` resource.
import PackageDescription

let package = Package(
    name: "SocktainerDNSImage",
    products: [
        .library(name: "SocktainerDNSImage", targets: ["SocktainerDNSImage"]),
    ],
    targets: [
        .target(
            name: "SocktainerDNSImage",
            resources: [
                // Byte-identical copy — never process/optimize an OCI archive.
                .copy("socktainer-dns.tar.gz"),
            ]
        ),
    ]
)
