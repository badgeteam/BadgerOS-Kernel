
# SPDX-License-Identifier: MIT

all: build

ARCH      ?= riscv64
MAKEFLAGS += --silent
SHELL     := /usr/bin/env bash
OUTPUT     = output

.PHONY: build
build:
	mkdir -p target
	echo "pub const RELEASE: &'static str = \"$$(echo -n $$(git describe --tags --always --dirty))\";" > target/version.rs
	echo "pub const VERSION: &'static str = \"$$(echo -n $$(date '+%Y-%m-%d %H:%M:%S %Z'))\";" >> target/version.rs
	cargo build \
		--target=misc/rust_target/$(ARCH)-kernel.json \
		--features=dtb,acpi,ktest

.PHONY: clean
clean:
	rm -rf '$(BUILDDIR)' '$(OUTPUT)'
