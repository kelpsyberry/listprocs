use super::utils::check_nonnull;
use libc::{pid_t, uid_t};
use std::{
    ffi::{CStr, OsStr, OsString},
    fmt, io,
    os::unix::ffi::OsStrExt,
    str::FromStr,
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(pub(super) pid_t);

impl Pid {
    pub fn raw(self) -> pid_t {
        self.0
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Pid {
    type Err = <pid_t as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        pid_t::from_str(s).map(Self)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid(pub(super) uid_t);

impl Uid {
    pub fn raw(self) -> uid_t {
        self.0
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Uid {
    type Err = <uid_t as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        uid_t::from_str(s).map(Self)
    }
}

impl Uid {
    pub fn current() -> Uid {
        unsafe { Uid(libc::getuid()) }
    }

    pub(super) fn username(self) -> io::Result<OsString> {
        unsafe {
            let passwd = check_nonnull(libc::getpwuid(self.0))?;
            Ok(OsStr::from_bytes(CStr::from_ptr((*passwd).pw_name).to_bytes()).to_os_string())
        }
    }
}
