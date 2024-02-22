use crate::{ffi::utils::check_pos, Pid};
use libc::{gid_t, uid_t, MAXCOMLEN};
use std::{
    ffi::{c_char, c_int},
    io,
    mem::{size_of, MaybeUninit},
};

const PROC_PIDT_SHORTBSDINFO: c_int = 13;

#[repr(C)]
#[doc(alias = "proc_bsdshortinfo")]
pub struct ProcBsdShortInfo {
    /// process id
    #[doc(alias = "pbsi_pid")]
    pub pid: u32,
    /// process parent id
    #[doc(alias = "pbsi_ppid")]
    pub parent_pid: u32,
    /// process perp id
    #[doc(alias = "pbsi_pgid")]
    pub process_group_id: u32,
    /// p_stat value, SZOMB, SRUN, etc
    #[doc(alias = "pbsi_status")]
    pub status: u32,
    /// upto 16 characters of process name
    #[doc(alias = "pbsi_comm")]
    pub name: [c_char; MAXCOMLEN],
    /// 64bit; emulated etc
    #[doc(alias = "pbsi_flags")]
    pub flags: u32,
    /// current uid on process
    #[doc(alias = "pbsi_uid")]
    pub uid: uid_t,
    /// current gid on process
    #[doc(alias = "pbsi_gid")]
    pub gid: gid_t,
    /// current ruid on process
    #[doc(alias = "pbsi_ruid")]
    pub real_uid: uid_t,
    /// current tgid on process
    #[doc(alias = "pbsi_rgid")]
    pub real_gid: gid_t,
    /// current svuid on process
    #[doc(alias = "pbsi_svuid")]
    pub saved_uid: uid_t,
    /// current svgid on process
    #[doc(alias = "pbsi_svgid")]
    pub saved_gid: gid_t,
    /// reserved for future use
    #[doc(alias = "pbsi_rfu")]
    pub reserved: u32,
}

impl Pid {
    pub(super) fn bsd_short_info(self) -> Result<ProcBsdShortInfo, io::Error> {
        unsafe {
            let mut result = MaybeUninit::<ProcBsdShortInfo>::uninit();
            check_pos(libc::proc_pidinfo(
                self.0,
                PROC_PIDT_SHORTBSDINFO,
                0,
                result.as_mut_ptr().cast(),
                size_of::<ProcBsdShortInfo>() as c_int,
            ))?;
            Ok(result.assume_init())
        }
    }
}
