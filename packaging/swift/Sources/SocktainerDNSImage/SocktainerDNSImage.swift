import Foundation

/// Accessor for the embedded `socktainer-dns` OCI image archive.
///
/// The gzipped OCI archive ships as a SwiftPM resource in this package's bundle.
/// socktainer loads it and imports the image into a guest VM at runtime — there is
/// no network fetch and no host execution of the binary.
public enum SocktainerDNSImage {
    /// Image reference baked into the archive (`buildah commit socktainer-dns:embedded`).
    public static let reference = "socktainer-dns:embedded"

    /// On-disk URL of the gzipped OCI archive resource.
    ///
    /// Force-unwrapped on purpose: the resource is committed alongside this source
    /// on every release tag, so its absence is a packaging error that must fail loudly.
    public static var archiveURL: URL {
        guard let url = Bundle.module.url(forResource: "socktainer-dns", withExtension: "tar.gz") else {
            fatalError("socktainer-dns.tar.gz is missing from the SocktainerDNSImage resource bundle")
        }
        return url
    }
}
