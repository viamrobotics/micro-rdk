# Docker targets that pre-cache go module downloads (intended to be rebuilt weekly/nightly)

MAIN_TAG = ghcr.io/viamrobotics/micro-rdk-dev-env

# The rust version used in these containers. When bumping this,
# reset `MAJOR_VER` to one and `MINOR_VER` to zero.
BUILD_TAG_RUST_VER=1.83.0

# The compatibility version for these images. Bump this when making
# incompatible changes to the images (e.g. changes for which there
# need to be compensating changes in the Micro-RDK, its templates,
# github actions, or generated projects, or when delivering a new
# feature in the image that will be relied on by micro-rdk. After
# pushing the images with `micro-rdk-upload`, adjust the pins in the
# Micro-RDK repository accordingly. The pins should not specify the
# minor version (e.g. use `1.83.0-v2`, not `1.83.0-v2.1`).
BUILD_TAG_MAJOR_VER=2

# The minor version. Update this if the newly built images can be used
# without modification. Since the pins are written against
# $(BUILD_TAG_MAJOR_VER) they will start using the new images
# immediately.
BUILD_TAG_MINOR_VER=0

BUILD_TAG_COMPAT_VER=$(BUILD_TAG_RUST_VER)-v$(BUILD_TAG_MAJOR_VER)
BUILD_TAG_FULL_VER=$(BUILD_TAG_RUST_VER)-v$(BUILD_TAG_MAJOR_VER).$(BUILD_TAG_MINOR_VER)

# TODO: Consider moving to a multi-platform image.
BUILD_CMD = docker buildx build \
		--pull $(BUILD_PUSH) \
		--build-arg MAIN_TAG=$(MAIN_TAG) \
		--build-arg BASE_TAG=$(BUILD_TAG) \
		--platform linux/$(BUILD_TAG) \
		-f $(BUILD_FILE) \
		-t '$(MAIN_TAG):$(BUILD_TAG)' \
		-t '$(MAIN_TAG):$(BUILD_TAG_COMPAT_VER)-$(BUILD_TAG)' \
		-t '$(MAIN_TAG):$(BUILD_TAG_FULL_VER)-$(BUILD_TAG)' \
		.

BUILD_PUSH = --load
BUILD_FILE = Dockerfile

micro-rdk-dev: micro-rdk-amd64 micro-rdk-arm64

micro-rdk-amd64: BUILD_TAG = amd64
micro-rdk-amd64:
	$(BUILD_CMD)

micro-rdk-arm64: BUILD_TAG = arm64
micro-rdk-arm64:
	$(BUILD_CMD)

micro-rdk-upload:
	docker push -a 'ghcr.io/viamrobotics/micro-rdk-dev-env:amd64'
	docker push -a 'ghcr.io/viamrobotics/micro-rdk-dev-env:arm64'
