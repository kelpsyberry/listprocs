pub use super::unix::*;

mod cmd_line;
mod proc_bsd_short_info;

use super::utils::{check_nonnull, check_pos, check_pos_zero};
use crate::{Info, ProcessInfo};
use libc::c_int;
use rayon::prelude::*;
use std::{
    ffi::{CStr, OsStr, OsString},
    io,
    mem::{size_of, MaybeUninit},
    os::unix::ffi::{OsStrExt, OsStringExt},
    ptr::null_mut,
    slice,
    time::{Duration, SystemTime},
};

#[repr(C)]
#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
struct mach_timebase_info {
    numer: u32,
    denom: u32,
}

extern "C" {
    fn devname(dev: libc::dev_t, mode: libc::mode_t) -> *mut libc::c_char;
    fn mach_timebase_info(info: *mut mach_timebase_info) -> c_int;
}

fn timebase_info() -> io::Result<mach_timebase_info> {
    Ok(memo!(mach_timebase_info, unsafe {
        let mut result = MaybeUninit::uninit();
        check_pos_zero(mach_timebase_info(result.as_mut_ptr()))?;
        result.assume_init()
    }))
}

fn ticks_to_duration(ticks: u128, timebase_info: mach_timebase_info) -> Duration {
    let nanos = ticks * timebase_info.numer as u128 / timebase_info.denom as u128;
    Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn physical_memory_max_size() -> io::Result<u64> {
    Ok(memo!(u64, {
        let mut result = 0;
        unsafe {
            check_pos_zero(libc::sysctl(
                [libc::CTL_HW, libc::HW_MEMSIZE].as_mut_ptr(),
                2,
                (&mut result as *mut u64).cast(),
                &mut size_of::<u64>(),
                null_mut(),
                0,
            ))?;
        }
        result
    }))
}

fn tty_name(dev: libc::dev_t) -> io::Result<OsString> {
    unsafe {
        let buf = check_nonnull(devname(dev, libc::S_IFCHR))?;
        Ok(OsStr::from_bytes(CStr::from_ptr(buf).to_bytes()).to_os_string())
    }
}

const PROC_PIDLISTTHREADS: c_int = 6;

impl Pid {
    pub fn all_active() -> io::Result<impl Iterator<Item = Self>> {
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

    fn path(self) -> io::Result<OsString> {
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

    fn proc_info<T, const FLAVOR: c_int>(self, arg: u64) -> io::Result<T> {
        unsafe {
            let mut result = MaybeUninit::<T>::uninit();
            check_pos(libc::proc_pidinfo(
                self.0,
                FLAVOR,
                arg,
                result.as_mut_ptr().cast(),
                size_of::<T>() as c_int,
            ))?;
            Ok(result.assume_init())
        }
    }

    fn list_threads(self, num_threads: usize) -> io::Result<Vec<u64>> {
        unsafe {
            let mut result = Vec::<u64>::with_capacity(num_threads);
            let len = check_pos(libc::proc_pidinfo(
                self.0,
                PROC_PIDLISTTHREADS,
                0,
                result.as_mut_ptr().cast(),
                (num_threads * size_of::<u64>()) as c_int,
            ))? as usize
                / size_of::<u64>();
            result.set_len(len);
            Ok(result)
        }
    }

    pub fn info(self) -> io::Result<ProcessInfo> {
        let bsd_short_info = match self.bsd_short_info() {
            Ok(info) => info,
            Err(err) => {
                if err.raw_os_error() == Some(3) {
                    return Ok(ProcessInfo {
                        is_defunct: true,
                        parent_pid: Info::Defunct,
                        uid: Info::Defunct,
                        username: Info::Defunct,
                        path: Info::Defunct,
                        cmd_line: Info::Defunct,
                        name: Info::Defunct,
                        cpu_usage: Info::Defunct,
                        cpu_time: Info::Defunct,
                        mem_usage: Info::Defunct,
                        virtual_mem_size: Info::Defunct,
                        physical_mem_size: Info::Defunct,
                        controlling_tty: Info::Defunct,
                        start_time: Info::Defunct,
                    });
                } else {
                    return Err(err);
                }
            }
        };

        let path = self.path()?;
        let path_str = path.to_string_lossy().into_owned();
        let uid = Uid(bsd_short_info.uid);
        let username = uid.username()?;
        let username_str = username.to_string_lossy().into_owned();
        let cmd_line = self.cmd_line()?;
        let cmd_line_str = cmd_line.map(|cmd_line_opt| {
            cmd_line_opt.map(|cmd_line| {
                cmd_line
                    .join(OsStr::new(" "))
                    .to_string_lossy()
                    .into_owned()
            })
        });
        let name = {
            let nul_index = bsd_short_info
                .name
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(libc::MAXCOMLEN);
            OsStr::from_bytes(unsafe {
                slice::from_raw_parts(bsd_short_info.name.as_ptr() as *const u8, nul_index)
            })
            .to_os_string()
        };
        let name_str = name.to_string_lossy().into_owned();

        let parent_pid = Pid(bsd_short_info.parent_pid as _);

        let bsd_task_info =
            match self.proc_info::<libc::proc_taskallinfo, { libc::PROC_PIDTASKALLINFO }>(0) {
                Ok(info) => info,
                Err(err) => {
                    if err.kind() == io::ErrorKind::PermissionDenied {
                        return Ok(ProcessInfo {
                            is_defunct: false,
                            parent_pid: Info::Some(parent_pid),
                            uid: Info::Some(uid),
                            username: Info::Some(username_str),
                            path: Info::Some(Some(path_str)),
                            cmd_line: cmd_line_str,
                            name: Info::Some(name_str),
                            cpu_usage: Info::Unauthorized,
                            cpu_time: Info::Unauthorized,
                            mem_usage: Info::Unauthorized,
                            virtual_mem_size: Info::Unauthorized,
                            physical_mem_size: Info::Unauthorized,
                            controlling_tty: Info::Unauthorized,
                            start_time: Info::Unauthorized,
                        });
                    } else {
                        return Err(err);
                    }
                }
            };

        let timebase_info = timebase_info()?;
        let start_time = SystemTime::UNIX_EPOCH
            + Duration::new(
                bsd_task_info.pbsd.pbi_start_tvsec,
                (bsd_task_info.pbsd.pbi_start_tvusec * 1000) as u32,
            );
        let cpu_time = ticks_to_duration(
            bsd_task_info.ptinfo.pti_total_user as u128
                + bsd_task_info.ptinfo.pti_total_system as u128,
            timebase_info,
        );
        let cpu_usage = {
            self.list_threads(bsd_task_info.ptinfo.pti_threadnum as usize)?
                .into_par_iter()
                .map(|thread| -> io::Result<i32> {
                    let thread_info = self
                        .proc_info::<libc::proc_threadinfo, { libc::PROC_PIDTHREADINFO }>(thread)?;
                    Ok(thread_info.pth_cpu_usage)
                })
                .sum::<io::Result<i32>>()? as f64
                / 1000.0
        };

        let physical_memory_max_size = physical_memory_max_size()?;
        let virtual_mem_size = bsd_task_info.ptinfo.pti_virtual_size;
        let physical_mem_size = bsd_task_info.ptinfo.pti_resident_size;
        let mem_usage = physical_mem_size as f64 / physical_memory_max_size as f64;

        let controlling_tty = if bsd_task_info.pbsd.e_tdev == u32::MAX {
            None
        } else {
            Some(tty_name(bsd_task_info.pbsd.e_tdev as _)?)
        };
        let controlling_tty_str = controlling_tty.map(|tty| tty.to_string_lossy().into_owned());

        Ok(ProcessInfo {
            is_defunct: false,
            parent_pid: Info::Some(parent_pid),
            uid: Info::Some(uid),
            username: Info::Some(username_str),
            path: Info::Some(Some(path_str)),
            cmd_line: cmd_line_str,
            name: Info::Some(name_str),
            cpu_usage: Info::Some(cpu_usage),
            cpu_time: Info::Some(cpu_time),
            mem_usage: Info::Some(mem_usage),
            virtual_mem_size: Info::Some(virtual_mem_size),
            physical_mem_size: Info::Some(physical_mem_size),
            controlling_tty: Info::Some(controlling_tty_str),
            start_time: Info::Some(start_time),
        })
    }
}
