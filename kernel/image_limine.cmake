
# Limine sources.
if("${CONFIG_CPU}" STREQUAL "riscv64")
set(limine_efi ${CMAKE_CURRENT_LIST_DIR}/lib/limine/BOOTRISCV64.EFI)
elseif("${CONFIG_CPU}" STREQUAL "x86_64")
set(limine_efi ${CMAKE_CURRENT_LIST_DIR}/lib/limine/BOOTX64.EFI)
endif()
set(limine_boot ${CMAKE_CURRENT_LIST_DIR}/limine.conf)
if("${CONFIG_CPU}" STREQUAL "x86_64")
    list(APPEND limine_boot ${CMAKE_CURRENT_LIST_DIR}/lib/limine/limine-bios.sys)
endif()

# Custom target that collects the files for the boot filesystem.
set(boot_dir ${CMAKE_BINARY_DIR}/boot.dir)
set(boot_fatfs ${CMAKE_BINARY_DIR}/boot.fatfs)
add_custom_command(
    OUTPUT ${boot_fatfs}
    
    # Populating the boot directory.
    COMMAND rm -rf ${boot_dir}
    COMMAND mkdir -p ${boot_dir}/EFI/BOOT
    COMMAND mkdir -p ${boot_dir}/boot
    COMMAND cp ${limine_efi} ${boot_dir}/EFI/BOOT/
    COMMAND cp ${limine_boot} ${boot_dir}/boot/
    COMMAND cp ${CMAKE_BINARY_DIR}/badger-os.stripped.elf ${boot_dir}/boot/badger-os.elf
    
    # Making a FAT filesystem image from it.
    COMMAND rm -f ${boot_fatfs}
    COMMAND dd if=/dev/zero bs=1M count=4 of=${boot_fatfs}
    COMMAND mformat -i ${boot_fatfs}
    COMMAND mcopy -s -i ${boot_fatfs} `find ${boot_dir}/ -mindepth 1 -maxdepth 1` ::/
    
    DEPENDS badger-os.stripped.elf ${limine_efi} ${limine_boot}
)

# Custom target that collects the files for the root filesystem.
set(root_dir ${CMAKE_CURRENT_LIST_DIR}/../files/root)
file(GLOB_RECURSE root_files "${root_dir}/*")
set(root_e2fs ${CMAKE_BINARY_DIR}/root.e2fs)
add_custom_command(
    OUTPUT ${root_e2fs}
    
    COMMAND rm -f ${root_e2fs}
    COMMAND dd if=/dev/zero bs=1M count=57 of=${root_e2fs}
    COMMAND fakeroot mkfs.ext2 -i 16384 -d ${root_dir} ${root_e2fs}
    
    DEPENDS ${root_files}
)

# Custom target for building the image.
set(image_iso ${CMAKE_BINARY_DIR}/image.iso)
if(CONFIG_EMBED_UBOOT)
    # Variant with U-boot embedded.
    add_custom_target(
        uboot.target
        make -C ${CMAKE_CURRENT_LIST_DIR}/lib/opensbi PLATFORM=generic CROSS_COMPILE=${CONFIG_PREFIX}
        COMMAND make -C ${CMAKE_CURRENT_LIST_DIR}/lib/u-boot sifive_unmatched_defconfig CROSS_COMPILE=${CONFIG_PREFIX}
        COMMAND make -C ${CMAKE_CURRENT_LIST_DIR}/lib/u-boot OPENSBI=${CMAKE_CURRENT_LIST_DIR}/lib/opensbi/build/platform/generic/firmware/fw_dynamic.bin CROSS_COMPILE=${CONFIG_PREFIX}
    )
    
    add_custom_command(
        OUTPUT ${image_iso}
        COMMAND rm -f ${image_iso}
        COMMAND dd if=/dev/zero bs=1M count=64 of=${image_iso}
        COMMAND
            sgdisk -a 1
                --new=1:34:2081    --change-name=1:spl   --typecode=1:5B193300-FC78-40CD-8002-E86C45580B47
                --new=2:2082:4095  --change-name=2:uboot --typecode=2:2E54B353-1271-4842-806F-E436D6AF6985
                --new=3:4096:12287 --change-name=3:boot  --typecode=3:0x0700
                --new=4:12288:-0   --change-name=4:root  --typecode=4:0x8300
                ${image_iso}
        COMMAND dd if=${CMAKE_CURRENT_LIST_DIR}/lib/u-boot/spl/u-boot-spl.bin bs=512 seek=34    of=${image_iso} conv=notrunc
        COMMAND dd if=${CMAKE_CURRENT_LIST_DIR}/lib/u-boot/u-boot.itb         bs=512 seek=2082  of=${image_iso} conv=notrunc
        COMMAND dd if=${boot_fatfs}                                           bs=512 seek=4096  of=${image_iso} conv=notrunc
        COMMAND dd if=${root_e2fs}                                            bs=512 seek=12288 of=${image_iso} conv=notrunc
        DEPENDS ${boot_fatfs} ${root_e2fs} uboot.target
    )
elseif("${CONFIG_CPU}" STREQUAL "x86_64")
    # Variant with Limine-BIOS embedded.
    add_custom_command(
        OUTPUT ${image_iso}
        COMMAND rm -f ${image_iso}
        COMMAND dd if=/dev/zero bs=1M count=64 of=${image_iso}
        COMMAND
            sgdisk -a 1
                --new=3:34:8225 --change-name=3:boot --typecode=3:0x0700
                --new=4:8226:-0 --change-name=4:root --typecode=4:0x8300
                ${image_iso}
        COMMAND dd if=${boot_fatfs} bs=512 seek=34   of=${image_iso} conv=notrunc
        COMMAND dd if=${root_e2fs}  bs=512 seek=8226 of=${image_iso} conv=notrunc
        COMMAND make -C ${CMAKE_CURRENT_LIST_DIR}/lib/limine
        COMMAND ${CMAKE_CURRENT_LIST_DIR}/lib/limine/limine bios-install ${image_iso}
        DEPENDS ${boot_fatfs} ${root_e2fs}
    )
else()
    # Variant without U-boot nor Limine-BIOS embedded.
    add_custom_command(
        OUTPUT ${image_iso}
        COMMAND rm -f ${image_iso}
        COMMAND dd if=/dev/zero bs=1M count=64 of=${image_iso}
        COMMAND
            sgdisk -a 1
                --new=3:34:8225 --change-name=3:boot --typecode=3:0x0700
                --new=4:8226:-0 --change-name=4:root --typecode=4:0x8300
                ${image_iso}
        COMMAND dd if=${boot_fatfs} bs=512 seek=34 of=${image_iso} conv=notrunc
        COMMAND dd if=${root_e2fs}  bs=512 seek=8226 of=${image_iso} conv=notrunc
        DEPENDS ${boot_fatfs} ${root_e2fs}
    )
endif()
add_custom_target(image.iso.target ALL DEPENDS ${image_iso})
install(FILES ${image_iso} DESTINATION .)
