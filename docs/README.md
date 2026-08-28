# BadgerOS documentation
This is the system administrator documentation for the BadgerOS kernel.

## Filesystems

### The devtmpfs
The devtmpfs is a RAM-backed filesystem typically mounted at `/dev` that allows for the creation of device files.

Devices can have a "node name"; a name by which they are automatically added to the devtmpfs upon registration.
For example, SATA drives have the node name `sata<index>` (where index is an integer no less than zero).

A `mount` call with the type `"devtmpfs"` always referes to the same instance,
that is, all filesystems mounted by a call with `source = NULL` and `fstype = "devtmpfs"` refer to the same filesystem.
Filesystem mount calls where `source` points to an existing mount of the devtmpfs can clone the mount that subsection of the devtmpfs as normal.

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

### Parameter: DUMPDTB
This parameter causes the kernel to dump the DTB to the log while booting.

### Parameter: ROOTWAIT
This parameter tells the kernel how long to wait for the root disk to be mounted, in seconds.
By default, the kernel waits 5 seconds.

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
| `ROOTDISK=<path>`        | Get the root disk by from devtmpfs

For the latter form, this refers to a path in the devtmpfs.
It must be a path referring to a block device, not a partition device.
For a specific partition, specify `ROOT=...`.

Example value: Booting from the first SATA drive: `ROOTDISK=ata0`.

If omitted, BadgerOS will look through all disks found, where the disk that the kernel was loaded from is first.

*See also: [Parameter: ROOT](#parameter-root) and [The devtmpfs](#the-devtmpfs).*

### Parameter: NO_AUTOMOUNT_DEVTMPFS
If this parameter is not specified, the devtmpfs is automatically mounted under `/dev` (if it is a legal mountpoint).

*See also: [The devtmpfs](#the-devtmpfs).*

### Parameter: DEVTMPFS_PATH
Change the location where the devtmpfs is automatically mounted (if `NO_AUTOMOUNT_DEVTMPFS` is not specified)

A default value of `DEVTMPFS_PATH=/dev` is implied.

*See also: [Parameter: NO_AUTOMOUNT_DEVTMPFS](#parameter-no_automount_devtmpfs) and [The devtmpfs](#the-devtmpfs).*

### Parameter: SYSCALL_TRACE
Causes every system call made to be logged.
