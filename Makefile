IMAGE_NAME  := socktainer-dns
IMAGE_TAG   := embedded
TARBALL     := $(IMAGE_NAME).tar

# Container engine for local image builds: docker (default), podman, or container.
# Override with `make build ENGINE=podman`.
ENGINE ?= docker

.PHONY: build tarball clean test bench

# Build the linux/arm64 OCI image (works on macOS and Linux).
# No musl toolchain required on the host — compilation happens inside the builder image.
build:
ifeq ($(ENGINE),docker)
	docker buildx build \
		--platform linux/arm64 \
		--load \
		-t $(IMAGE_NAME):$(IMAGE_TAG) .
else
	$(ENGINE) build \
		--platform linux/arm64 \
		-t $(IMAGE_NAME):$(IMAGE_TAG) .
endif

# Export the image as a gzipped OCI archive (the artifact consumed by socktainer).
tarball:
ifeq ($(ENGINE),docker)
	# `--output type=oci` writes a real OCI layout tarball in one command, so a
	# failed build aborts instead of leaving a truncated .gz.
	docker buildx build \
		--platform linux/arm64 \
		--output type=oci,dest=$(TARBALL) \
		-t $(IMAGE_NAME):$(IMAGE_TAG) .
else
	# podman / container: build, then export an OCI archive (mirrors the CI
	# `buildah push oci-archive:` flow). Verify the push syntax for your engine.
	$(ENGINE) build \
		--platform linux/arm64 \
		-t $(IMAGE_NAME):$(IMAGE_TAG) .
	$(ENGINE) push $(IMAGE_NAME):$(IMAGE_TAG) oci-archive:$(TARBALL)
endif
	gzip -f $(TARBALL)
	@echo "Tarball: $(TARBALL).gz ($$(wc -c < $(TARBALL).gz | tr -d ' ') bytes)"

# Run unit tests (native platform — no Docker or musl required).
test:
	cargo test

# Run criterion benchmarks (native platform).
bench:
	cargo bench

clean:
	rm -f $(TARBALL).gz
