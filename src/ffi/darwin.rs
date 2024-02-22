pub use super::unix::*;

mod cmd_line;
mod proc_bsd_short_info;

use super::utils::check_pos;
use crate::{Info, ProcessInfo};
use std::{
    ffi::{OsStr, OsString},
    io,
    mem::size_of,
    os::unix::ffi::OsStringExt,
    ptr::null_mut,
};

impl Pid {
    pub fn all_active() -> Result<impl Iterator<Item = Self>, io::Error> {
        unsafe {
            // NOTE: Assumes `repr(transparent)` for `Pid`.
            let mut len = check_pos(libc::proc_listallpids(null_mut(), 0))? as usize;
            let mut result = Vec::<Pid>::with_capacity(len);
            len = check_pos(libc::proc_listallpids(
                result.as_mut_ptr().cast(),
                (len * size_of::<Pid>()) as i32,
            ))? as usize;
            result.set_len(len);
            Ok(result.into_iter().filter(|&pid| pid.0 != 0))
        }
    }

    fn path(self) -> Result<OsString, io::Error> {
        unsafe {
            let mut path = Vec::<u8>::with_capacity(libc::PROC_PIDPATHINFO_MAXSIZE as usize);
            let path_len = check_pos(libc::proc_pidpath(
                self.0,
                path.as_mut_ptr().cast(),
                libc::PROC_PIDPATHINFO_MAXSIZE as u32,
            ))? as usize;
            path.set_len(path_len);
            path.shrink_to_fit();
            Ok(std::ffi::OsString::from_vec(path))
        }
    }

    pub fn info(self) -> Result<ProcessInfo, io::Error> {
        let bsd_info = match self.bsd_short_info() {
            Ok(info) => info,
            Err(err) => {
                if err.raw_os_error() == Some(3) {
                    return Ok(ProcessInfo {
                        is_defunct: true,
                        parent_pid: Info::Defunct,
                        uid: Info::Defunct,
                        username: Info::Defunct,
                        cmd_line: Info::Defunct,
                        path: Info::Defunct,
                    });
                } else {
                    return Err(err);
                }
            }
        };
        let path = self.path()?;
        let uid = Uid(bsd_info.uid);
        let username = uid.username()?;
        let cmd_line = self.cmd_line()?;

        Ok(ProcessInfo {
            is_defunct: false,
            parent_pid: Info::Some(Pid(bsd_info.parent_pid as _)),
            uid: Info::Some(uid),
            username: Info::Some(username.to_string_lossy().into_owned()),
            path: Info::Some(path.to_string_lossy().into_owned()),
            cmd_line: match cmd_line {
                Info::Defunct => Info::Defunct,
                Info::Unauthorized => Info::Unauthorized,
                Info::Some(cmd_line) => {
                    Info::Some(cmd_line.map(|cmd_line| {
                        cmd_line.join(OsStr::new(" ")).to_string_lossy().to_string()
                    }))
                }
            },
        })
    }
}
