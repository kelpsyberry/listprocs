use crate::{ffi::utils::check_pos_zero, Info, Pid};
use std::{
    ffi::{c_int, OsStr, OsString},
    io,
    mem::size_of,
    os::unix::ffi::OsStrExt,
    ptr::null_mut,
};

impl Pid {
    pub(super) fn cmd_line(self) -> Result<Info<Option<Vec<OsString>>>, io::Error> {
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
                [libc::CTL_KERN, libc::KERN_PROCARGS2, self.0 as c_int].as_mut_ptr(),
                3,
                args_mem.as_mut_ptr().cast(),
                &mut args_mem_len,
                null_mut(),
                0,
            )) {
                match err.kind() {
                    io::ErrorKind::InvalidInput => return Ok(Info::Unauthorized),
                    _ => return Err(err),
                }
            }
            args_mem.set_len(args_mem_len);

            let arg_count = args_mem.as_ptr().cast::<u32>().read_unaligned() as usize;

            let mut start = 4;
            while start < args_mem.len() && args_mem[start] != 0 {
                start += 1;
            }
            while start < args_mem.len() && args_mem[start] == 0 {
                start += 1;
            }
            if start == args_mem.len() {
                return Ok(Info::Some(None));
            }

            let mut args = Vec::with_capacity(arg_count);
            let mut cur_arg_start = start;
            for cur in start..args_mem.len() {
                if args_mem[cur] != 0 {
                    continue;
                }
                args.push(OsStr::from_bytes(&args_mem[cur_arg_start..cur]).to_os_string());
                if args.len() >= arg_count {
                    break;
                }
                cur_arg_start = cur + 1;
            }
            Ok(Info::Some((!args.is_empty()).then_some(args)))
        }
    }
}
