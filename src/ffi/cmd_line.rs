use super::{utils::check_pos_zero, Pid};
use std::{
    ffi::{c_int, CStr, CString},
    io,
    mem::size_of,
    ptr::{self, null_mut},
};

#[derive(Clone, Debug)]
pub enum CmdLine<S> {
    None,
    Unauthorized,
    Some(S),
}

impl CmdLine<CString> {
    pub fn for_pid(pid: Pid) -> Result<Self, io::Error> {
        unsafe {
            let mut args_mem_len: c_int = 0;
            check_pos_zero(libc::sysctl(
                [libc::CTL_KERN, libc::KERN_ARGMAX].as_mut_ptr(),
                2,
                (&mut args_mem_len as *mut c_int).cast(),
                &mut size_of::<c_int>(),
                null_mut(),
                0,
            ))?;
            let mut args_mem_len = args_mem_len as usize;
            let mut args_mem = Vec::<u8>::with_capacity(args_mem_len);
            if let Err(err) = check_pos_zero(libc::sysctl(
                [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as c_int].as_mut_ptr(),
                3,
                args_mem.as_mut_ptr().cast(),
                &mut args_mem_len,
                null_mut(),
                0,
            )) {
                match err.kind() {
                    io::ErrorKind::InvalidInput => return Ok(CmdLine::Unauthorized),
                    _ => return Err(err),
                }
            }
            args_mem.set_len(args_mem_len);

            let arg_count: u32 = ptr::read_unaligned(args_mem.as_ptr().cast());

            let mut start = 4;
            while start < args_mem.len() && args_mem[start] != 0 {
                start += 1;
            }
            while start < args_mem.len() && args_mem[start] == 0 {
                start += 1;
            }
            if start == args_mem.len() {
                return Ok(CmdLine::None);
            }

            let end = {
                let mut arg_i = 0;
                let mut cur = start;
                let mut last_end = None;
                while arg_i < arg_count && cur < args_mem.len() {
                    if args_mem[cur] == 0 {
                        arg_i += 1;
                        if let Some(last_end) = last_end {
                            args_mem[last_end] = b' ';
                        }
                        last_end = Some(cur);
                    }
                    cur += 1;
                }
                let Some(last_end) = last_end else {
                    return Ok(CmdLine::None);
                };
                last_end
            };

            Ok(CmdLine::Some(
                CStr::from_bytes_with_nul(&args_mem[start..=end])
                    .expect("string should have been null terminated")
                    .to_owned(),
            ))
        }
    }
}
