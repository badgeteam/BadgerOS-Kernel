
# SPDX-License-Identifier: MIT

MAKEFLAGS += --silent -j$(shell nproc)
SHELL     := /usr/bin/env bash

.PHONY: all
all: build

.PHONY: selarch
selarch:
	./tools/selarch.py

.PHONY: gui-config
gui-config: cmake-configure
	cmake-gui -S kernel -B kernel/build

.PHONY: config
config: cmake-configure
	ccmake kernel -B kernel/build
	# ./tools/config.py
	# git submodule update --init
	# $(MAKE) -C kernel _on_config

.PHONY: hh24_defconfig
hh24_defconfig:
	./tools/config.py --target esp32c6 --use-default
	$(MAKE) -C kernel _on_config

.PHONY: why2025_defconfig
why2025_defconfig:
	./tools/config.py --target esp32p4 --use-default
	$(MAKE) -C kernel _on_config

.PHONY: unmatched_defconfig
unmatched_defconfig:
	./tools/config.py --target generic --use-default --vec_spec none
	$(MAKE) -C kernel _on_config

.PHONY: build
build:
	$(MAKE) -C files build
	$(MAKE) -C kernel build

.PHONY: clean
clean:
	$(MAKE) -C files clean
	$(MAKE) -C kernel clean

.PHONY: cmake-configure
cmake-configure:
	$(MAKE) -C files build
	$(MAKE) -C kernel cmake-configure

.PHONY: openocd
openocd:
	$(MAKE) -C kernel openocd

.PHONY: gdb
gdb:
	$(MAKE) -C kernel gdb

.PHONY: qemu
qemu:
	$(MAKE) -C files build
	$(MAKE) -C kernel qemu

.PHONY: qemu-debug
qemu-debug:
	$(MAKE) -C files build
	$(MAKE) -C kernel qemu-debug

.PHONY: qemu-prof
qemu-prof:
	$(MAKE) -C files build
	$(MAKE) -C kernel qemu-prof

.PHONY: flash
flash:
	$(MAKE) -C files build
	$(MAKE) -C kernel flash

.PHONY: image
image:
	$(MAKE) -C files build
	$(MAKE) -C kernel image

.PHONY: burn
burn:
	$(MAKE) -C files build
	$(MAKE) -C kernel burn

.PHONY: monitor
monitor:
	$(MAKE) -C kernel monitor

.PHONY: clang-format-check
clang-format-check:
	$(MAKE) -C kernel clang-format-check

.PHONY: clang-tidy-check
clang-tidy-check:
	$(MAKE) -C kernel clang-tidy-check
