#ifndef CSOCKTAINER_DNS_IMAGE_H
#define CSOCKTAINER_DNS_IMAGE_H

// Bytes of the gzipped OCI archive, embedded in the binary's data segment.
// `embedded_dns_image.c` (with the byte array) is generated at release time by
// the workflow from the freshly built tarball — see .github/workflows/release.yml.
const unsigned char *socktainer_dns_image_bytes(void);
unsigned int socktainer_dns_image_len(void);

#endif
