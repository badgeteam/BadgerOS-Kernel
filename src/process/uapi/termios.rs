// SPDX-FileCopyrightText: 2026 Julian Scheffers <julian@scheffers.net>
// SPDX-FileType: SOURCE
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use core::ffi::c_uint;

use crate::process::usercopy::UserCopyable;

pub type cc_t = u8;
pub type speed_t = c_uint;
pub type tcflag_t = c_uint;

pub const NCCS: usize = 32;

// c_iflag
pub const IGNBRK: tcflag_t = 0o000001;
pub const BRKINT: tcflag_t = 0o000002;
pub const IGNPAR: tcflag_t = 0o000004;
pub const PARMRK: tcflag_t = 0o000010;
pub const INPCK: tcflag_t = 0o000020;
pub const ISTRIP: tcflag_t = 0o000040;
pub const INLCR: tcflag_t = 0o000100;
pub const IGNCR: tcflag_t = 0o000200;
pub const ICRNL: tcflag_t = 0o000400;
pub const IUCLC: tcflag_t = 0o001000;
pub const IXON: tcflag_t = 0o002000;
pub const IXANY: tcflag_t = 0o004000;
pub const IXOFF: tcflag_t = 0o010000;
pub const IMAXBEL: tcflag_t = 0o020000;
pub const IUTF8: tcflag_t = 0o040000;

// c_oflag
pub const OPOST: tcflag_t = 0o000001;
pub const OLCUC: tcflag_t = 0o000002;
pub const ONLCR: tcflag_t = 0o000004;
pub const OCRNL: tcflag_t = 0o000010;
pub const ONOCR: tcflag_t = 0o000020;
pub const ONLRET: tcflag_t = 0o000040;
pub const OFILL: tcflag_t = 0o000100;
pub const OFDEL: tcflag_t = 0o000200;

// c_cflag
pub const CSIZE: tcflag_t = 0o000060;
pub const CS5: tcflag_t = 0o000000;
pub const CS6: tcflag_t = 0o000020;
pub const CS7: tcflag_t = 0o000040;
pub const CS8: tcflag_t = 0o000060;

pub const CSTOPB: tcflag_t = 0o000100;
pub const CREAD: tcflag_t = 0o000200;
pub const PARENB: tcflag_t = 0o000400;
pub const PARODD: tcflag_t = 0o001000;
pub const HUPCL: tcflag_t = 0o002000;
pub const CLOCAL: tcflag_t = 0o004000;

// c_lflag
pub const ISIG: tcflag_t = 0o000001;
pub const ICANON: tcflag_t = 0o000002;
pub const ECHO: tcflag_t = 0o000010;
pub const ECHOE: tcflag_t = 0o000020;
pub const ECHOK: tcflag_t = 0o000040;
pub const ECHONL: tcflag_t = 0o000100;
pub const NOFLSH: tcflag_t = 0o000200;
pub const TOSTOP: tcflag_t = 0o000400;
pub const IEXTEN: tcflag_t = 0o100000;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct termios {
    pub c_iflag: tcflag_t,
    pub c_oflag: tcflag_t,
    pub c_cflag: tcflag_t,
    pub c_lflag: tcflag_t,
    pub c_line: cc_t,
    pub c_cc: [cc_t; NCCS],
    pub c_ibaud: speed_t,
    pub c_obaud: speed_t,
}
unsafe impl UserCopyable for termios {}

impl Default for termios {
    fn default() -> Self {
        Self {
            c_iflag: ICRNL,
            c_oflag: Default::default(),
            c_cflag: Default::default(),
            c_lflag: ECHO,
            c_line: Default::default(),
            c_cc: Default::default(),
            c_ibaud: Default::default(),
            c_obaud: Default::default(),
        }
    }
}
