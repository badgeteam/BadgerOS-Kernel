
# SPDX-License-Identifier: MIT

all: build

CONFIG_PATH        = ../.config/config.mk
include $(CONFIG_PATH)
include cpu/$(CONFIG_CPU)/cpu.mk

MAKEFLAGS         += --silent -j$(shell nproc)
SHELL             := /usr/bin/env bash
OUTPUT             = $(shell pwd)/output
BUILDDIR           = $(shell pwd)/build
BADGER_RAMFS_ROOT ?= $(shell pwd)/../files/root
CROSS_COMPILE      = $(CONFIG_PREFIX)
GDB               ?= $(CROSS_COMPILE)gdb
CLANG_FORMAT      ?= clang-format-18
CLANG_TIDY        ?= clang-tidy-18
DRIVE             ?= /dev/null
PORT              ?= /dev/ttyUSB1

.PHONY: cmake-configure
cmake-configure:
	mkdir -p '$(BUILDDIR)'
	cmake -B '$(BUILDDIR)'

.PHONY: _on_config
_on_config: _port_on_config
	git submodule update --init lib/kbelf

.PHONY: build
build: cmake-configure
	cmake --build '$(BUILDDIR)'
	cmake --install '$(BUILDDIR)' --prefix '$(OUTPUT)'

.PHONY: clean
clean:
	rm -rf '$(BUILDDIR)' '$(OUTPUT)'

.PHONY: clang-format-check
clang-format-check: cmake-configure
	echo "clang-format check the following files:"
	jq -r '.[].file' build/compile_commands.json | grep '\.[ch]$$'
	echo "analysis results:"
	$(CLANG_FORMAT) --dry-run $(shell jq -r '.[].file' build/compile_commands.json | grep '\.[ch]$$')

.PHONY: clang-tidy-check
clang-tidy-check: cmake-configure
	echo "clang-tidy check the following files:"
	jq -r '.[].file' build/compile_commands.json | grep '\.[ch]$$'
	echo "analysis results:"
	$(CLANG_TIDY) -p build $(shell jq -r '.[].file' build/compile_commands.json | grep '\.[ch]$$') \
		--checks=-*widening*multiplication*,-*insecureAPI* \
		--warnings-as-errors="*"

.PHONY: gdb
gdb:
	$(GDB) -x misc/gdbinit build/badger-os.elf

.PHONY: burn
burn: build
	sudo dd if=$(OUTPUT)/image.iso of=$(DRIVE) bs=1M oflag=sync conv=nocreat

.PHONY: monitor
monitor:
	echo -e "\033[1mType ^A^X to exit.\033[0m"
	picocom -q -b 115200 '$(PORT)' \
	| ../tools/address-filter.py -A $(CROSS_COMPILE)addr2line '$(OUTPUT)/badger-os.elf'; echo -e '\033[0m'
