
# SPDX-License-Identifier: MIT

all: build

ARCH      ?= riscv64
MAKEFLAGS += --silent
SHELL     := /usr/bin/env bash
OUTPUT     = output/$(ARCH)

.PHONY: build
build:
	cargo -Z unstable-options build \
		--artifact-dir=$(OUTPUT) \
		--target=misc/rust_target/$(ARCH)-kernel.json \
		--features=dtb,acpi,ktest

.PHONY: clean
clean:
	rm -rf '$(BUILDDIR)' '$(OUTPUT)'
