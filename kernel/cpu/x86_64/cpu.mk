
QEMU          ?= qemu-system-x86_64

.PHONY: qemu
qemu-debug: build
	$(QEMU) -s -S \
		-d int -no-reboot -no-shutdown \
		-smp 1 -m 1G -cpu max,tsc-frequency=1000000000 \
		-device pcie-root-port,bus=pci.0,id=pcisw0 \
		-device qemu-xhci,bus=pcisw0 -device usb-kbd \
		-device virtio-scsi-pci,id=scsi \
		-drive id=hd0,format=raw,file=$(OUTPUT)/image.iso \
		-serial mon:stdio -display none \
	| ../tools/address-filter.py -L -A $(CROSS_COMPILE)addr2line $(OUTPUT)/badger-os.elf

.PHONY: qemu
qemu: build
	$(QEMU) -s \
		-smp 1 -m 1G -cpu max,tsc-frequency=1000000000 \
		-device pcie-root-port,bus=pci.0,id=pcisw0 \
		-device qemu-xhci,bus=pcisw0 -device usb-kbd \
		-device virtio-scsi-pci,id=scsi \
		-drive id=hd0,format=raw,file=$(OUTPUT)/image.iso \
		-serial mon:stdio -display none \
	| ../tools/address-filter.py -L -A $(CROSS_COMPILE)addr2line $(OUTPUT)/badger-os.elf

.PHONY: qemu-prof
qemu-prof: build
	$(QEMU) --plugin $(QEMU_PLUGINS)/libuftrace.so -s \
		-smp 1 -m 1G -cpu max,tsc-frequency=1000000000 \
		-device pcie-root-port,bus=pci.0,id=pcisw0 \
		-device qemu-xhci,bus=pcisw0 -device usb-kbd \
		-device virtio-scsi-pci,id=scsi \
		-drive id=hd0,format=raw,file=$(OUTPUT)/image.iso \
		-serial mon:stdio -display none \
	| ../tools/address-filter.py -L -A $(CROSS_COMPILE)addr2line $(OUTPUT)/badger-os.elf
	$(QEMU_SOURCE)/contrib/plugins/uftrace_symbols.py --prefix-symbols $(OUTPUT)/badger-os.elf
	uftrace dump --chrome > chromedump.json
