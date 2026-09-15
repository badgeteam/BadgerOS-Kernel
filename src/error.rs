use core::{error::Error, fmt::Display, str};

use alloc::{alloc::AllocError, collections::TryReserveError};
#[cfg(feature = "dtb")]
use dtb::DtbError;

use crate::abi;

/// Errno enum that matches those of BadgerOS.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errno {
    EPERM = abi::EPERM,
    ENOENT = abi::ENOENT,
    ESRCH = abi::ESRCH,
    EINTR = abi::EINTR,
    EIO = abi::EIO,
    ENXIO = abi::ENXIO,
    E2BIG = abi::E2BIG,
    ENOEXEC = abi::ENOEXEC,
    EBADF = abi::EBADF,
    ECHILD = abi::ECHILD,
    EAGAIN = abi::EAGAIN,
    ENOMEM = abi::ENOMEM,
    EACCES = abi::EACCES,
    EFAULT = abi::EFAULT,
    ENOTBLK = abi::ENOTBLK,
    EBUSY = abi::EBUSY,
    EEXIST = abi::EEXIST,
    EXDEV = abi::EXDEV,
    ENODEV = abi::ENODEV,
    ENOTDIR = abi::ENOTDIR,
    EISDIR = abi::EISDIR,
    EINVAL = abi::EINVAL,
    ENFILE = abi::ENFILE,
    EMFILE = abi::EMFILE,
    ENOTTY = abi::ENOTTY,
    ETXTBSY = abi::ETXTBSY,
    EFBIG = abi::EFBIG,
    ENOSPC = abi::ENOSPC,
    ESPIPE = abi::ESPIPE,
    EROFS = abi::EROFS,
    EMLINK = abi::EMLINK,
    EPIPE = abi::EPIPE,
    EDOM = abi::EDOM,
    ERANGE = abi::ERANGE,

    EDEADLK = abi::EDEADLK,
    ENAMETOOLONG = abi::ENAMETOOLONG,
    ENOLCK = abi::ENOLCK,

    ENOSYS = abi::ENOSYS,

    ENOTEMPTY = abi::ENOTEMPTY,
    ELOOP = abi::ELOOP,
    ENOTSUP = abi::ENOTSUP,
    ENOMSG = abi::ENOMSG,
    EIDRM = abi::EIDRM,
    ECHRNG = abi::ECHRNG,
    EL2NSYNC = abi::EL2NSYNC,
    EL3HLT = abi::EL3HLT,
    EL3RST = abi::EL3RST,
    ELNRNG = abi::ELNRNG,
    EUNATCH = abi::EUNATCH,
    ENOCSI = abi::ENOCSI,
    EL2HLT = abi::EL2HLT,
    EBADE = abi::EBADE,
    EBADR = abi::EBADR,
    EXFULL = abi::EXFULL,
    ENOANO = abi::ENOANO,
    EBADRQC = abi::EBADRQC,
    EBADSLT = abi::EBADSLT,

    EBFONT = abi::EBFONT,
    ENOSTR = abi::ENOSTR,
    ENODATA = abi::ENODATA,
    ETIME = abi::ETIME,
    ENOSR = abi::ENOSR,
    ENONET = abi::ENONET,
    ENOPKG = abi::ENOPKG,
    EREMOTE = abi::EREMOTE,
    ENOLINK = abi::ENOLINK,
    EADV = abi::EADV,
    ESRMNT = abi::ESRMNT,
    ECOMM = abi::ECOMM,
    EPROTO = abi::EPROTO,
    EMULTIHOP = abi::EMULTIHOP,
    EDOTDOT = abi::EDOTDOT,
    EBADMSG = abi::EBADMSG,
    EOVERFLOW = abi::EOVERFLOW,
    ENOTUNIQ = abi::ENOTUNIQ,
    EBADFD = abi::EBADFD,
    EREMCHG = abi::EREMCHG,
    ELIBACC = abi::ELIBACC,
    ELIBBAD = abi::ELIBBAD,
    ELIBSCN = abi::ELIBSCN,
    ELIBMAX = abi::ELIBMAX,
    ELIBEXEC = abi::ELIBEXEC,
    EILSEQ = abi::EILSEQ,
    ERESTART = abi::ERESTART,
    ESTRPIPE = abi::ESTRPIPE,
    EUSERS = abi::EUSERS,
    ENOTSOCK = abi::ENOTSOCK,
    EDESTADDRREQ = abi::EDESTADDRREQ,
    EMSGSIZE = abi::EMSGSIZE,
    EPROTOTYPE = abi::EPROTOTYPE,
    ENOPROTOOPT = abi::ENOPROTOOPT,
    EPROTONOSUPPORT = abi::EPROTONOSUPPORT,
    ESOCKTNOSUPPORT = abi::ESOCKTNOSUPPORT,
    // EOPNOTSUPP duplicates ENOTSUPP
    EPFNOSUPPORT = abi::EPFNOSUPPORT,
    EAFNOSUPPORT = abi::EAFNOSUPPORT,
    EADDRINUSE = abi::EADDRINUSE,
    EADDRNOTAVAIL = abi::EADDRNOTAVAIL,
    ENETDOWN = abi::ENETDOWN,
    ENETUNREACH = abi::ENETUNREACH,
    ENETRESET = abi::ENETRESET,
    ECONNABORTED = abi::ECONNABORTED,
    ECONNRESET = abi::ECONNRESET,
    ENOBUFS = abi::ENOBUFS,
    EISCONN = abi::EISCONN,
    ENOTCONN = abi::ENOTCONN,
    ESHUTDOWN = abi::ESHUTDOWN,
    ETOOMANYREFS = abi::ETOOMANYREFS,
    ETIMEDOUT = abi::ETIMEDOUT,
    ECONNREFUSED = abi::ECONNREFUSED,
    EHOSTDOWN = abi::EHOSTDOWN,
    EHOSTUNREACH = abi::EHOSTUNREACH,
    EALREADY = abi::EALREADY,
    EINPROGRESS = abi::EINPROGRESS,
    ESTALE = abi::ESTALE,
    EUCLEAN = abi::EUCLEAN,
    ENOTNAM = abi::ENOTNAM,
    ENAVAIL = abi::ENAVAIL,
    EISNAM = abi::EISNAM,
    EREMOTEIO = abi::EREMOTEIO,
    EDQUOT = abi::EDQUOT,

    ENOMEDIUM = abi::ENOMEDIUM,
    EMEDIUMTYPE = abi::EMEDIUMTYPE,
    ECANCELED = abi::ECANCELED,
    ENOKEY = abi::ENOKEY,
    EKEYEXPIRED = abi::EKEYEXPIRED,
    EKEYREVOKED = abi::EKEYREVOKED,
    EKEYREJECTED = abi::EKEYREJECTED,

    EOWNERDEAD = abi::EOWNERDEAD,
    ENOTRECOVERABLE = abi::ENOTRECOVERABLE,
    ERFKILL = abi::ERFKILL,
    EHWPOISON = abi::EHWPOISON,

    // BadgerOS-specific:
    EASSERT = abi::EASSERT,
    EALIGN = abi::EALIGN,
}

impl Errno {
    /// Get the name of this errno.
    pub fn name(&self) -> &'static str {
        match self {
            Self::EPERM => "EPERM",
            Self::ENOENT => "ENOENT",
            Self::ESRCH => "ESRCH",
            Self::EINTR => "EINTR",
            Self::EIO => "EIO",
            Self::ENXIO => "ENXIO",
            Self::E2BIG => "E2BIG",
            Self::ENOEXEC => "ENOEXEC",
            Self::EBADF => "EBADF",
            Self::ECHILD => "ECHILD",
            Self::EAGAIN => "EAGAIN",
            Self::ENOMEM => "ENOMEM",
            Self::EACCES => "EACCES",
            Self::EFAULT => "EFAULT",
            Self::ENOTBLK => "ENOTBLK",
            Self::EBUSY => "EBUSY",
            Self::EEXIST => "EEXIST",
            Self::EXDEV => "EXDEV",
            Self::ENODEV => "ENODEV",
            Self::ENOTDIR => "ENOTDIR",
            Self::EISDIR => "EISDIR",
            Self::EINVAL => "EINVAL",
            Self::ENFILE => "ENFILE",
            Self::EMFILE => "EMFILE",
            Self::ENOTTY => "ENOTTY",
            Self::ETXTBSY => "ETXTBSY",
            Self::EFBIG => "EFBIG",
            Self::ENOSPC => "ENOSPC",
            Self::ESPIPE => "ESPIPE",
            Self::EROFS => "EROFS",
            Self::EMLINK => "EMLINK",
            Self::EPIPE => "EPIPE",
            Self::EDOM => "EDOM",
            Self::ERANGE => "ERANGE",
            Self::EDEADLK => "EDEADLK",
            Self::ENAMETOOLONG => "ENAMETOOLONG",
            Self::ENOLCK => "ENOLCK",
            Self::ENOSYS => "ENOSYS",
            Self::ENOTEMPTY => "ENOTEMPTY",
            Self::ELOOP => "ELOOP",
            Self::ENOTSUP => "ENOTSUP",
            Self::ENOMSG => "ENOMSG",
            Self::EIDRM => "EIDRM",
            Self::ECHRNG => "ECHRNG",
            Self::EL2NSYNC => "EL2NSYNC",
            Self::EL3HLT => "EL3HLT",
            Self::EL3RST => "EL3RST",
            Self::ELNRNG => "ELNRNG",
            Self::EUNATCH => "EUNATCH",
            Self::ENOCSI => "ENOCSI",
            Self::EL2HLT => "EL2HLT",
            Self::EBADE => "EBADE",
            Self::EBADR => "EBADR",
            Self::EXFULL => "EXFULL",
            Self::ENOANO => "ENOANO",
            Self::EBADRQC => "EBADRQC",
            Self::EBADSLT => "EBADSLT",
            Self::EBFONT => "EBFONT",
            Self::ENOSTR => "ENOSTR",
            Self::ENODATA => "ENODATA",
            Self::ETIME => "ETIME",
            Self::ENOSR => "ENOSR",
            Self::ENONET => "ENONET",
            Self::ENOPKG => "ENOPKG",
            Self::EREMOTE => "EREMOTE",
            Self::ENOLINK => "ENOLINK",
            Self::EADV => "EADV",
            Self::ESRMNT => "ESRMNT",
            Self::ECOMM => "ECOMM",
            Self::EPROTO => "EPROTO",
            Self::EMULTIHOP => "EMULTIHOP",
            Self::EDOTDOT => "EDOTDOT",
            Self::EBADMSG => "EBADMSG",
            Self::EOVERFLOW => "EOVERFLOW",
            Self::ENOTUNIQ => "ENOTUNIQ",
            Self::EBADFD => "EBADFD",
            Self::EREMCHG => "EREMCHG",
            Self::ELIBACC => "ELIBACC",
            Self::ELIBBAD => "ELIBBAD",
            Self::ELIBSCN => "ELIBSCN",
            Self::ELIBMAX => "ELIBMAX",
            Self::ELIBEXEC => "ELIBEXEC",
            Self::EILSEQ => "EILSEQ",
            Self::ERESTART => "ERESTART",
            Self::ESTRPIPE => "ESTRPIPE",
            Self::EUSERS => "EUSERS",
            Self::ENOTSOCK => "ENOTSOCK",
            Self::EDESTADDRREQ => "EDESTADDRREQ",
            Self::EMSGSIZE => "EMSGSIZE",
            Self::EPROTOTYPE => "EPROTOTYPE",
            Self::ENOPROTOOPT => "ENOPROTOOPT",
            Self::EPROTONOSUPPORT => "EPROTONOSUPPORT",
            Self::ESOCKTNOSUPPORT => "ESOCKTNOSUPPORT",
            Self::EPFNOSUPPORT => "EPFNOSUPPORT",
            Self::EAFNOSUPPORT => "EAFNOSUPPORT",
            Self::EADDRINUSE => "EADDRINUSE",
            Self::EADDRNOTAVAIL => "EADDRNOTAVAIL",
            Self::ENETDOWN => "ENETDOWN",
            Self::ENETUNREACH => "ENETUNREACH",
            Self::ENETRESET => "ENETRESET",
            Self::ECONNABORTED => "ECONNABORTED",
            Self::ECONNRESET => "ECONNRESET",
            Self::ENOBUFS => "ENOBUFS",
            Self::EISCONN => "EISCONN",
            Self::ENOTCONN => "ENOTCONN",
            Self::ESHUTDOWN => "ESHUTDOWN",
            Self::ETOOMANYREFS => "ETOOMANYREFS",
            Self::ETIMEDOUT => "ETIMEDOUT",
            Self::ECONNREFUSED => "ECONNREFUSED",
            Self::EHOSTDOWN => "EHOSTDOWN",
            Self::EHOSTUNREACH => "EHOSTUNREACH",
            Self::EALREADY => "EALREADY",
            Self::EINPROGRESS => "EINPROGRESS",
            Self::ESTALE => "ESTALE",
            Self::EUCLEAN => "EUCLEAN",
            Self::ENOTNAM => "ENOTNAM",
            Self::ENAVAIL => "ENAVAIL",
            Self::EISNAM => "EISNAM",
            Self::EREMOTEIO => "EREMOTEIO",
            Self::EDQUOT => "EDQUOT",
            Self::ENOMEDIUM => "ENOMEDIUM",
            Self::EMEDIUMTYPE => "EMEDIUMTYPE",
            Self::ECANCELED => "ECANCELED",
            Self::ENOKEY => "ENOKEY",
            Self::EKEYEXPIRED => "EKEYEXPIRED",
            Self::EKEYREVOKED => "EKEYREVOKED",
            Self::EKEYREJECTED => "EKEYREJECTED",
            Self::EOWNERDEAD => "EOWNERDEAD",
            Self::ENOTRECOVERABLE => "ENOTRECOVERABLE",
            Self::ERFKILL => "ERFKILL",
            Self::EHWPOISON => "EHWPOISON",
            // BadgerOS-specific:
            Self::EASSERT => "EASSERT",
            Self::EALIGN => "EALIGN",
        }
    }

    /// Get a brief description of this errno.
    pub fn desc(&self) -> &'static str {
        match self {
            Self::EPERM => "Operation not permitted",
            Self::ENOENT => "No such file or directory",
            Self::ESRCH => "No such process",
            Self::EINTR => "Interrupted system call",
            Self::EIO => "I/O error",
            Self::ENXIO => "No such device or address",
            Self::E2BIG => "Argument list too long",
            Self::ENOEXEC => "Exec format error",
            Self::EBADF => "Bad file number",
            Self::ECHILD => "No child processes",
            Self::EAGAIN => "Try again",
            Self::ENOMEM => "Out of memory",
            Self::EACCES => "Permission denied",
            Self::EFAULT => "Bad address",
            Self::ENOTBLK => "Block device required",
            Self::EBUSY => "Device or resource busy",
            Self::EEXIST => "File exists",
            Self::EXDEV => "Cross-device link",
            Self::ENODEV => "No such device",
            Self::ENOTDIR => "Not a directory",
            Self::EISDIR => "Is a directory",
            Self::EINVAL => "Invalid argument",
            Self::ENFILE => "File table overflow",
            Self::EMFILE => "Too many open files",
            Self::ENOTTY => "Not a typewriter",
            Self::ETXTBSY => "Text file busy",
            Self::EFBIG => "File too large",
            Self::ENOSPC => "No space left on device",
            Self::ESPIPE => "Illegal seek",
            Self::EROFS => "Read-only file system",
            Self::EMLINK => "Too many links",
            Self::EPIPE => "Broken pipe",
            Self::EDOM => "Math argument out of domain of func",
            Self::ERANGE => "Math result not representable",
            Self::EDEADLK => "Resource deadlock would occur",
            Self::ENAMETOOLONG => "File name too long",
            Self::ENOLCK => "No locks available",
            Self::ENOSYS => "Function not implemented",
            Self::ENOTEMPTY => "Directory not empty",
            Self::ELOOP => "Too many symbolic links encountered",
            Self::ENOTSUP => "Not supported",
            Self::ENOMSG => "No message of desired type",
            Self::EIDRM => "Identifier removed",
            Self::ECHRNG => "Channel number out of range",
            Self::EL2NSYNC => "Level 2 not synchronized",
            Self::EL3HLT => "Level 3 halted",
            Self::EL3RST => "Level 3 reset",
            Self::ELNRNG => "Link number out of range",
            Self::EUNATCH => "Protocol driver not attached",
            Self::ENOCSI => "No CSI structure available",
            Self::EL2HLT => "Level 2 halted",
            Self::EBADE => "Invalid exchange",
            Self::EBADR => "Invalid request descriptor",
            Self::EXFULL => "Exchange full",
            Self::ENOANO => "No anode",
            Self::EBADRQC => "Invalid request code",
            Self::EBADSLT => "Invalid slot",
            Self::EBFONT => "Bad font file format",
            Self::ENOSTR => "Device not a stream",
            Self::ENODATA => "No data available",
            Self::ETIME => "Timer expired",
            Self::ENOSR => "Out of streams resources",
            Self::ENONET => "Machine is not on the network",
            Self::ENOPKG => "Package not installed",
            Self::EREMOTE => "Object is remote",
            Self::ENOLINK => "Link has been severed",
            Self::EADV => "Advertise error",
            Self::ESRMNT => "Srmount error",
            Self::ECOMM => "Communication error on send",
            Self::EPROTO => "Protocol error",
            Self::EMULTIHOP => "Multihop attempted",
            Self::EDOTDOT => "RFS specific error",
            Self::EBADMSG => "Not a data message",
            Self::EOVERFLOW => "Value too large for defined data type",
            Self::ENOTUNIQ => "Name not unique on network",
            Self::EBADFD => "File descriptor in bad state",
            Self::EREMCHG => "Remote address changed",
            Self::ELIBACC => "Can not access a needed shared library",
            Self::ELIBBAD => "Accessing a corrupted shared library",
            Self::ELIBSCN => ".lib section in a.out corrupted",
            Self::ELIBMAX => "Attempting to link in too many shared libraries",
            Self::ELIBEXEC => "Cannot exec a shared library directly",
            Self::EILSEQ => "Illegal byte sequence",
            Self::ERESTART => "Interrupted system call should be restarted",
            Self::ESTRPIPE => "Streams pipe error",
            Self::EUSERS => "Too many users",
            Self::ENOTSOCK => "Socket operation on non-socket",
            Self::EDESTADDRREQ => "Destination address required",
            Self::EMSGSIZE => "Message too long",
            Self::EPROTOTYPE => "Protocol wrong type for socket",
            Self::ENOPROTOOPT => "Protocol not available",
            Self::EPROTONOSUPPORT => "Protocol not supported",
            Self::ESOCKTNOSUPPORT => "Socket type not supported",
            Self::EPFNOSUPPORT => "Protocol family not supported",
            Self::EAFNOSUPPORT => "Address family not supported by protocol",
            Self::EADDRINUSE => "Address already in use",
            Self::EADDRNOTAVAIL => "Cannot assign requested address",
            Self::ENETDOWN => "Network is down",
            Self::ENETUNREACH => "Network is unreachable",
            Self::ENETRESET => "Network dropped connection because of reset",
            Self::ECONNABORTED => "Software caused connection abort",
            Self::ECONNRESET => "Connection reset by peer",
            Self::ENOBUFS => "No buffer space available",
            Self::EISCONN => "Transport endpoint is already connected",
            Self::ENOTCONN => "Transport endpoint is not connected",
            Self::ESHUTDOWN => "Cannot send after transport endpoint shutdown",
            Self::ETOOMANYREFS => "Too many references: cannot splice",
            Self::ETIMEDOUT => "Operation timed out",
            Self::ECONNREFUSED => "Connection refused",
            Self::EHOSTDOWN => "Host is down",
            Self::EHOSTUNREACH => "No route to host",
            Self::EALREADY => "Operation already in progress",
            Self::EINPROGRESS => "Operation now in progress",
            Self::ESTALE => "Stale file handle",
            Self::EUCLEAN => "Structure needs cleaning",
            Self::ENOTNAM => "Not a XENIX named type file",
            Self::ENAVAIL => "No XENIX semaphores available",
            Self::EISNAM => "Is a named type file",
            Self::EREMOTEIO => "Remote I/O error",
            Self::EDQUOT => "Quota exceeded",
            Self::ENOMEDIUM => "No medium found",
            Self::EMEDIUMTYPE => "Wrong medium type",
            Self::ECANCELED => "Operation Canceled",
            Self::ENOKEY => "Required key not available",
            Self::EKEYEXPIRED => "Key has expired",
            Self::EKEYREVOKED => "Key has been revoked",
            Self::EKEYREJECTED => "Key was rejected by service",
            Self::EOWNERDEAD => "Owner died",
            Self::ENOTRECOVERABLE => "State not recoverable",
            Self::ERFKILL => "Operation not possible due to RF-kill",
            Self::EHWPOISON => "Memory page has hardware error",
            // BadgerOS-specific:
            Self::EASSERT => "Assertion failed",
            Self::EALIGN => "Address misaligned",
        }
    }

    /// Create an `EResult` from some integer.
    pub fn check_bool(errno: i32) -> EResult<bool> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute(-errno) })
        } else {
            Ok(errno != 0)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_i32(errno: i32) -> EResult<i32> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute(-errno) })
        } else {
            Ok(errno as i32)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_i64(errno: i64) -> EResult<i64> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute((-errno) as u32) })
        } else {
            Ok(errno as i64)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_u32(errno: i32) -> EResult<u32> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute(-errno) })
        } else {
            Ok(errno as u32)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_u64(errno: i64) -> EResult<u64> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute((-errno) as u32) })
        } else {
            Ok(errno as u64)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_isize(errno: isize) -> EResult<isize> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute((-errno) as u32) })
        } else {
            Ok(errno as isize)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check_usize(errno: isize) -> EResult<usize> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute((-errno) as u32) })
        } else {
            Ok(errno as usize)
        }
    }
    /// Create an `EResult` from some integer.
    pub fn check(errno: i32) -> EResult<()> {
        if errno < 0 {
            Err(unsafe { core::mem::transmute(-errno) })
        } else {
            Ok(())
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_i32(res: EResult<i32>) -> i32 {
        match res {
            Ok(x) => x as i32,
            Err(x) => -(x as u32 as i32),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_i64(res: EResult<i64>) -> i64 {
        match res {
            Ok(x) => x as i64,
            Err(x) => -(x as u64 as i64),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_u32(res: EResult<u32>) -> i32 {
        match res {
            Ok(x) => x as i32,
            Err(x) => -(x as u32 as i32),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_u64(res: EResult<u64>) -> i64 {
        match res {
            Ok(x) => x as i64,
            Err(x) => -(x as u64 as i64),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_bool(res: EResult<bool>) -> i32 {
        match res {
            Ok(x) => x as i32,
            Err(x) => -(x as u32 as i32),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract(res: EResult<()>) -> i32 {
        match res {
            Ok(()) => 0,
            Err(x) => -(x as u32 as i32),
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_isize(res: EResult<isize>) -> isize {
        match res {
            Ok(x) => x as isize,
            Err(x) => -(x as u32 as isize),
        }
    }
    /// Convert an `EResult` into a raw pointer.
    pub fn extract_ptr<T>(res: EResult<*mut T>) -> *mut T {
        match res {
            Ok(x) => x,
            Err(x) => -(x as u32 as isize) as *mut T,
        }
    }
    /// Convert an `EResult` into an integer.
    pub fn extract_usize(res: EResult<usize>) -> isize {
        match res {
            Ok(x) => x as isize,
            Err(x) => -(x as u32 as isize),
        }
    }
}

impl Display for Errno {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} ({})", self.name(), self.desc())
    }
}

impl Error for Errno {}

impl From<AllocError> for Errno {
    fn from(_: AllocError) -> Self {
        Errno::ENOMEM
    }
}

impl From<TryReserveError> for Errno {
    fn from(_: TryReserveError) -> Self {
        Errno::ENOMEM
    }
}

#[cfg(feature = "dtb")]
impl From<DtbError> for Errno {
    fn from(value: DtbError) -> Self {
        match value {
            DtbError::Invalid => Errno::EINVAL,
            DtbError::NoMemory => Errno::ENOMEM,
        }
    }
}

macro_rules! errno {
    ($errno: tt) => {
        crate::error::Errno::$errno
    };
}

pub type EResult<T> = Result<T, Errno>;
