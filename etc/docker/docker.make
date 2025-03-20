# Docker targets that pre-cache go module downloads (intended to be rebuilt weekly/nightly)
BUILD_CMD = docker buildx build --pull $(BUILD_PUSH) --build-arg MAIN_TAG=$(MAIN_TAG) --build-arg BASE_TAG=$(BUILD_TAG) --platform linux/$(BUILD_TAG) -f $(BUILD_FILE) -t '$(MAIN_TAG):$(BUILD_TAG)' -t '$(MAIN_TAG):$(BUILD_TAG_VER)-$(BUILD_TAG)' .
BUILD_PUSH = --load
BUILD_FILE = Dockerfile


micro-rdk-dev: micro-rdk-amd64 micro-rdk-arm64
micro-rdk-amd64: MAIN_TAG = ghcr.io/viamrobotics/micro-rdk-dev-env
micro-rdk-amd64: BUILD_TAG = amd64
micro-rdk-amd64:
ifndef DOCKER_RUST_VERSION
micro-rdk-amd64: BUILD_TAG_VER = latest
else
micro-rdk-amd64: BUILD_TAG_VER = $(DOCKER_RUST_VERSION)
endif
micro-rdk-amd64:
	$(BUILD_CMD)

micro-rdk-arm64: MAIN_TAG = ghcr.io/viamrobotics/micro-rdk-dev-env
micro-rdk-arm64: BUILD_TAG = arm64
micro-rdk-arm64:
ifndef DOCKER_RUST_VERSION
micro-rdk-arm64: BUILD_TAG_VER = latest
else
micro-rdk-arm64: BUILD_TAG_VER = $(DOCKER_RUST_VERSION)
endif
micro-rdk-arm64:
	$(BUILD_CMD)

micro-rdk-upload:
	docker push -a 'ghcr.io/viamrobotics/micro-rdk-dev-env'
