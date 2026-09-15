
# SPDX-License-Identifier: MIT

all: build

ARCH         ?= riscv64
MAKEFLAGS    += --silent
SHELL        := /usr/bin/env bash
BUILD_DIR    ?= target
ARTIFACT_DIR ?= output/$(ARCH)

.PHONY: build
build:
	cargo -Z unstable-options build \
		--target-dir=$(BUILD_DIR) \
		--artifact-dir=$(ARTIFACT_DIR) \
		--target=misc/rust_target/$(ARCH)-kernel.json \
		--features=dtb,acpi,ktest

.PHONY: clean
clean:
	cargo clean
