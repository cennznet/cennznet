$(warning $(shell IMAGE_NAME=$(IMAGE_NAME) printenv | grep IMAGE_NAME))
$(warning $(shell IMAGE_TAG=$(IMAGE_TAG) printenv | grep IMAGE_TAG))
$(warning $(shell DOCKER_BUILD_ARGS=$(DOCKER_BUILD_ARGS) printenv | grep DOCKER_BUILD_ARGS))

ifndef IMAGE_NAME
#$(warning IMAGE_NAME is not set)
IMAGE_NAME=cennznet
endif

ifndef DOCKER_BUILD_ARGS
# You can add '--no-cache --quite' here if you like
DOCKER_BUILD_ARGS=
endif

ifndef IMAGE_TAG
IMAGE_TAG=$(shell date '+%Y%m%d%H')
endif

build:
	docker build $(DOCKER_BUILD_ARGS) -t $(IMAGE_NAME):$(IMAGE_TAG) .

all: build
