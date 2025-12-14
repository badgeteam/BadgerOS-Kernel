use core::{error::Error, fmt::Display, str};

use alloc::{alloc::AllocError, collections::TryReserveError};

use super::raw;

/// Signal enum that matches those of BadgerOS.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    SIGHUP = raw::SIGHUP,
    SIGINT = raw::SIGINT,
    SIGQUIT = raw::SIGQUIT,
    SIGILL = raw::SIGILL,
    SIGTRAP = raw::SIGTRAP,
    SIGABRT = raw::SIGABRT,
    SIGBUS = raw::SIGBUS,
    SIGFPE = raw::SIGFPE,
    SIGKILL = raw::SIGKILL,
    SIGUSR1 = raw::SIGUSR1,
    SIGSEGV = raw::SIGSEGV,
    SIGUSR2 = raw::SIGUSR2,
    SIGPIPE = raw::SIGPIPE,
    SIGALRM = raw::SIGALRM,
    SIGTERM = raw::SIGTERM,
    SIGSTKFLT = raw::SIGSTKFLT,
    SIGCHLD = raw::SIGCHLD,
    SIGCONT = raw::SIGCONT,
    SIGSTOP = raw::SIGSTOP,
    SIGTSTP = raw::SIGTSTP,
    SIGTTIN = raw::SIGTTIN,
    SIGTTOU = raw::SIGTTOU,
    SIGURG = raw::SIGURG,
    SIGXCPU = raw::SIGXCPU,
    SIGXFSZ = raw::SIGXFSZ,
    SIGVTALRM = raw::SIGVTALRM,
    SIGPROF = raw::SIGPROF,
    SIGWINCH = raw::SIGWINCH,
    SIGIO = raw::SIGIO,
    SIGPWR = raw::SIGPWR,
    SIGSYS = raw::SIGSYS,
}

/// Errno enum that matches those of BadgerOS.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errno {
    EPERM = raw::EPERM,
    ENOENT = raw::ENOENT,
    ESRCH = raw::ESRCH,
    EINTR = raw::EINTR,
    EIO = raw::EIO,
    ENXIO = raw::ENXIO,
    E2BIG = raw::E2BIG,
    ENOEXEC = raw::ENOEXEC,
    EBADF = raw::EBADF,
    ECHILD = raw::ECHILD,
    EAGAIN = raw::EAGAIN,
    ENOMEM = raw::ENOMEM,
    EACCES = raw::EACCES,
    EFAULT = raw::EFAULT,
    ENOTBLK = raw::ENOTBLK,
    EBUSY = raw::EBUSY,
    EEXIST = raw::EEXIST,
    EXDEV = raw::EXDEV,
    ENODEV = raw::ENODEV,
    ENOTDIR = raw::ENOTDIR,
    EISDIR = raw::EISDIR,
    EINVAL = raw::EINVAL,
    ENFILE = raw::ENFILE,
    EMFILE = raw::EMFILE,
    ENOTTY = raw::ENOTTY,
    ETXTBSY = raw::ETXTBSY,
    EFBIG = raw::EFBIG,
    ENOSPC = raw::ENOSPC,
    ESPIPE = raw::ESPIPE,
    EROFS = raw::EROFS,
    EMLINK = raw::EMLINK,
    EPIPE = raw::EPIPE,
    EDOM = raw::EDOM,
    ERANGE = raw::ERANGE,

    EDEADLK = raw::EDEADLK,
    ENAMETOOLONG = raw::ENAMETOOLONG,
    ENOLCK = raw::ENOLCK,

    ENOSYS = raw::ENOSYS,

    ENOTEMPTY = raw::ENOTEMPTY,
    ELOOP = raw::ELOOP,
    ENOTSUP = raw::ENOTSUP,
    ENOMSG = raw::ENOMSG,
    EIDRM = raw::EIDRM,
    ECHRNG = raw::ECHRNG,
    EL2NSYNC = raw::EL2NSYNC,
    EL3HLT = raw::EL3HLT,
    EL3RST = raw::EL3RST,
    ELNRNG = raw::ELNRNG,
    EUNATCH = raw::EUNATCH,
    ENOCSI = raw::ENOCSI,
    EL2HLT = raw::EL2HLT,
    EBADE = raw::EBADE,
    EBADR = raw::EBADR,
    EXFULL = raw::EXFULL,
    ENOANO = raw::ENOANO,
    EBADRQC = raw::EBADRQC,
    EBADSLT = raw::EBADSLT,

    EBFONT = raw::EBFONT,
    ENOSTR = raw::ENOSTR,
    ENODATA = raw::ENODATA,
    ETIME = raw::ETIME,
    ENOSR = raw::ENOSR,
    ENONET = raw::ENONET,
    ENOPKG = raw::ENOPKG,
    EREMOTE = raw::EREMOTE,
    ENOLINK = raw::ENOLINK,
    EADV = raw::EADV,
    ESRMNT = raw::ESRMNT,
    ECOMM = raw::ECOMM,
    EPROTO = raw::EPROTO,
    EMULTIHOP = raw::EMULTIHOP,
    EDOTDOT = raw::EDOTDOT,
    EBADMSG = raw::EBADMSG,
    EOVERFLOW = raw::EOVERFLOW,
    ENOTUNIQ = raw::ENOTUNIQ,
    EBADFD = raw::EBADFD,
    EREMCHG = raw::EREMCHG,
    ELIBACC = raw::ELIBACC,
    ELIBBAD = raw::ELIBBAD,
    ELIBSCN = raw::ELIBSCN,
    ELIBMAX = raw::ELIBMAX,
    ELIBEXEC = raw::ELIBEXEC,
    EILSEQ = raw::EILSEQ,
    ERESTART = raw::ERESTART,
    ESTRPIPE = raw::ESTRPIPE,
    EUSERS = raw::EUSERS,
    ENOTSOCK = raw::ENOTSOCK,
    EDESTADDRREQ = raw::EDESTADDRREQ,
    EMSGSIZE = raw::EMSGSIZE,
    EPROTOTYPE = raw::EPROTOTYPE,
    ENOPROTOOPT = raw::ENOPROTOOPT,
    EPROTONOSUPPORT = raw::EPROTONOSUPPORT,
    ESOCKTNOSUPPORT = raw::ESOCKTNOSUPPORT,
    EOPNOTSUPP = raw::EOPNOTSUPP,
    EPFNOSUPPORT = raw::EPFNOSUPPORT,
    EAFNOSUPPORT = raw::EAFNOSUPPORT,
    EADDRINUSE = raw::EADDRINUSE,
    EADDRNOTAVAIL = raw::EADDRNOTAVAIL,
    ENETDOWN = raw::ENETDOWN,
    ENETUNREACH = raw::ENETUNREACH,
    ENETRESET = raw::ENETRESET,
    ECONNABORTED = raw::ECONNABORTED,
    ECONNRESET = raw::ECONNRESET,
    ENOBUFS = raw::ENOBUFS,
    EISCONN = raw::EISCONN,
    ENOTCONN = raw::ENOTCONN,
    ESHUTDOWN = raw::ESHUTDOWN,
    ETOOMANYREFS = raw::ETOOMANYREFS,
    ETIMEDOUT = raw::ETIMEDOUT,
    ECONNREFUSED = raw::ECONNREFUSED,
    EHOSTDOWN = raw::EHOSTDOWN,
    EHOSTUNREACH = raw::EHOSTUNREACH,
    EALREADY = raw::EALREADY,
    EINPROGRESS = raw::EINPROGRESS,
    ESTALE = raw::ESTALE,
    EUCLEAN = raw::EUCLEAN,
    ENOTNAM = raw::ENOTNAM,
    ENAVAIL = raw::ENAVAIL,
    EISNAM = raw::EISNAM,
    EREMOTEIO = raw::EREMOTEIO,
    EDQUOT = raw::EDQUOT,

    ENOMEDIUM = raw::ENOMEDIUM,
    EMEDIUMTYPE = raw::EMEDIUMTYPE,
    ECANCELED = raw::ECANCELED,
    ENOKEY = raw::ENOKEY,
    EKEYEXPIRED = raw::EKEYEXPIRED,
    EKEYREVOKED = raw::EKEYREVOKED,
    EKEYREJECTED = raw::EKEYREJECTED,

    EOWNERDEAD = raw::EOWNERDEAD,
    ENOTRECOVERABLE = raw::ENOTRECOVERABLE,
    ERFKILL = raw::ERFKILL,
    EHWPOISON = raw::EHWPOISON,

    EASSERT = raw::EASSERT,
}

impl Errno {
    /// Get the name of this errno.
    pub fn name(&self) -> &'static str {
        unsafe {
            let cs = raw::errno_get_name(*self as i32);
            str::from_raw_parts(cs as *const u8, raw::strlen(cs))
        }
    }
    /// Get a brief description of this errno.
    pub fn desc(&self) -> &'static str {
        unsafe {
            let cs = raw::errno_get_desc(*self as i32);
            str::from_raw_parts(cs as *const u8, raw::strlen(cs))
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

impl Error for Errno {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }

    fn provide<'a>(&'a self, _: &mut core::error::Request<'a>) {}
}

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

macro_rules! errno {
    ($errno: tt) => {
        crate::bindings::error::Errno::$errno
    };
}

pub type EResult<T> = Result<T, Errno>;
