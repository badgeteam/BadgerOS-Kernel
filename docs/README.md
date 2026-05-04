# BadgerOS documentation
This documentation is largely a stub, sorry :^)

## Kernel parameters
BadgerOS supports key-value parameters specified by the boot protocol.
Some parameters are optional, but there may never be a duplicate parameter.
If a duplicate parameter is encountered, BadgerOS will print a warning but try to boot anyway.

### Data type: GUID/UUID
BadgerOS supports 128-bit GUIDs/UUIDs of the following forms, which it parses little-endian:
- `{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}`
- `(xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)`
- `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
- `{xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}`
- `(xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx)`
- `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`

Where `x` is any of the following ASCII characters representing a hexadecimal encoding: 0-9, a-f and A-F.

### Parameter: ROOT
This parameter specifies how to mount the root filesystem.
It can take the following forms:

| format              | description
| :------------------ | :----------
| `ROOT=PARTUUID=...` | The first partition found with this partition [UUID](#data-type-guiduuid)
| `ROOT=PARTTYPE=...` | The first partition found with this type [UUID](#data-type-guiduuid)
| `ROOT=PART=...`     | A zero-indexed decimal partition number on the root disk
| `ROOT=WHOLEDISK`    | Use the entirety of `ROOTDISK` to mount the root filesystem

A default value of `ROOT=PARTTYPE=0FC63DAF-8483-4772-8E79-3D69D8477DE4` is implied.

*Note: If `ROOTDISK` is not specified and `ROOT=PART=...` then **only the disk that the kernel is loaded from is considered**.*

*See also: [Parameter: ROOTDISK](#parameter-rootdisk).*

### Parameter: ROOTDISK
Restricts which disks to search when looking for the root partition.
It can take the following forms:
| format                   | description
| :----------------------- | :----------
| `ROOTDISK=UUID=...`      | Get the root disk by disk [UUID](#data-type-guiduuid)
| `ROOTDISK=<type><index>` | Get the root disk by type and index

For the latter form, `type` is the block device driver's type code (e.g. `sata`) and `index` is the decimal zero-based index of the disk (the first disk of a type will get the lowest index).

Example value: Booting from the first SATA drive: `ROOTDISK=sata0`.

If omitted, BadgerOS will look through all disks found, where the disk that the kernel was loaded from is first.

*See also: [Parameter: ROOT](#parameter-root).*
