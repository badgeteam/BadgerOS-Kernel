
QEMU          ?= qemu-system-riscv64

$(BUILDDIR)/cache/OVMF_RISCV64.fd:
	mkdir -p $(BUILDDIR)/cache
	test -f $(BUILDDIR)/cache/OVMF_RISCV64.fd || ( \
		cd $(BUILDDIR)/cache \
		&& curl -o OVMF_RISCV64.fd https://retrage.github.io/edk2-nightly/bin/RELEASERISCV64_VIRT_CODE.fd \
		&& dd if=/dev/zero of=OVMF_RISCV64.fd bs=1 count=0 seek=33554432 \
	)

.PHONY: qemu
qemu-debug: $(BUILDDIR)/cache/OVMF_RISCV64.fd build
	$(QEMU) -s -S \
		-icount shift=auto,sleep=off -rtc clock=vm,base=utc \
		-M virt,acpi=off -cpu rv64,sv48=false -smp 4 -m 1G \
		-device pcie-root-port,bus=pcie.0,id=pcisw0 \
		-device qemu-xhci,bus=pcisw0 -device usb-kbd \
		-drive if=pflash,unit=0,format=raw,file=$(BUILDDIR)/cache/OVMF_RISCV64.fd \
		-drive if=none,id=hd0,format=raw,file=$(OUTPUT)/image.iso,cache=none \
		-device ahci,id=achi0 \
		-device ide-hd,drive=hd0,bus=achi0.0 \
		-serial mon:stdio -nographic \
	| ../tools/address-filter.py -L -A $(CROSS_COMPILE)addr2line $(OUTPUT)/badger-os.elf

.PHONY: qemu
qemu: $(BUILDDIR)/cache/OVMF_RISCV64.fd build
	$(QEMU) -s \
		-M virt,acpi=off -cpu rv64,sv48=false -smp 4 -m 1G \
		-device pcie-root-port,bus=pcie.0,id=pcisw0 \
		-device qemu-xhci,bus=pcisw0 -device usb-kbd \
		-drive if=pflash,unit=0,format=raw,file=$(BUILDDIR)/cache/OVMF_RISCV64.fd \
		-drive if=none,id=hd0,format=raw,file=$(OUTPUT)/image.iso,cache=none \
		-device ahci,id=achi0 \
		-device ide-hd,drive=hd0,bus=achi0.0 \
		-serial mon:stdio -nographic \
	| ../tools/address-filter.py -L -A $(CROSS_COMPILE)addr2line $(OUTPUT)/badger-os.elf
