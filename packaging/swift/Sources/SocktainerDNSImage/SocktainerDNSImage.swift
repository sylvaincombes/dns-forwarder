import CSocktainerDNSImage
import Foundation

/// Accessor for the embedded `socktainer-dns` OCI image archive.
///
/// The gzipped OCI archive is compiled into the binary by `CSocktainerDNSImage`
/// (its bytes live in the executable's data segment), so a standalone `socktainer`
/// binary carries the image with no external resource bundle to ship or lose.
public enum SocktainerDNSImage {
    /// Image reference baked into the archive (`buildah commit socktainer-dns:embedded`).
    public static let reference = "socktainer-dns:embedded"

    /// The gzipped OCI archive bytes, read directly from the binary's data segment.
    public static var archiveData: Data {
        Data(bytes: socktainer_dns_image_bytes(), count: Int(socktainer_dns_image_len()))
    }

    /// On-disk URL of the archive for callers that import from a path.
    ///
    /// Written fresh and atomically on every call: fresh so an upgraded binary never
    /// reuses a previous version's file at the shared name, atomic so a reader never
    /// sees a torn or partially written archive.
    public static func archiveURL() throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("socktainer-dns-embedded.tar.gz")
        try archiveData.write(to: url, options: .atomic)
        return url
    }
}
