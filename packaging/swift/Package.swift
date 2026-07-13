// swift-tools-version: 6.0
//
// SwiftPM distribution package for the socktainer-dns OCI image.
//
// `main` is the Rust source. On each release the workflow builds the OCI tarball,
// generates `Sources/CSocktainerDNSImage/embedded_dns_image.c` from it (the image
// bytes as a C array), copies these package files onto the `swiftpm` branch, and
// tags that commit `vX.Y.Z`. socktainer consumes a version with:
//
//     .package(url: "https://github.com/socktainer/dns-forwarder.git", exact: "X.Y.Z")
//
// The archive is compiled into the binary (CSocktainerDNSImage), not shipped as a
// resource bundle, so a standalone `socktainer` executable carries the image with
// no co-located `.bundle` to lose (see socktainer issue #316).
import PackageDescription

let package = Package(
    name: "SocktainerDNSImage",
    products: [
        .library(name: "SocktainerDNSImage", targets: ["SocktainerDNSImage"]),
    ],
    targets: [
        .target(name: "CSocktainerDNSImage"),
        .target(
            name: "SocktainerDNSImage",
            dependencies: ["CSocktainerDNSImage"]
        ),
    ]
)
