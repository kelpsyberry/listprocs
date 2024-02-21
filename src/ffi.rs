pub mod proc_bsd_short_info;
mod utils;
pub use proc_bsd_short_info::ProcBsdShortInfo;
mod cmd_line;
pub use cmd_line::CmdLine;

use std::{
    ffi::{CStr, CString},
    io,
    mem::size_of,
    ptr::null_mut,
};
use utils::{check_nonnull, check_pos};

pub type Pid = libc::pid_t;
pub type Uid = libc::uid_t;

pub fn all_pids() -> Result<Vec<Pid>, io::Error> {
    unsafe {
        let mut len = check_pos(libc::proc_listallpids(null_mut(), 0))? as usize;
        let mut result = Vec::<Pid>::with_capacity(len);
        len = check_pos(libc::proc_listallpids(
            result.as_mut_ptr().cast(),
            (len * size_of::<Pid>()) as i32,
        ))? as usize;
        result.set_len(len);
        Ok(result)
    }
}

pub fn path_for_pid(pid: Pid) -> Result<CString, io::Error> {
    unsafe {
        let mut path = Vec::<u8>::with_capacity(libc::PROC_PIDPATHINFO_MAXSIZE as usize);
        let path_len = check_pos(libc::proc_pidpath(
            pid,
            path.as_mut_ptr().cast(),
            libc::PROC_PIDPATHINFO_MAXSIZE as u32,
        ))? as usize
            + 1;
        path.set_len(path_len);
        Ok(std::ffi::CString::from_vec_with_nul(path)
            .expect("string should have been null terminated"))
    }
}

pub fn current_uid() -> Uid {
    unsafe { libc::getuid() }
}

pub fn username_for_uid(uid: Uid) -> Result<CString, io::Error> {
    unsafe {
        let passwd = check_nonnull(libc::getpwuid(uid))?;
        Ok(CStr::from_ptr((*passwd).pw_name).to_owned())
    }
}
