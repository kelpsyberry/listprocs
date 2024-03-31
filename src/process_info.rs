use crate::{Info, Pid, Uid};

use rayon::prelude::*;
use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct ProcessInfo {
    pub is_defunct: bool,
    pub parent_pid: Info<Pid>,
    pub uid: Info<Uid>,
    pub username: Info<String>,
    pub path: Info<Option<String>>,
    pub cmd_line: Info<Option<String>>,
    pub name: Info<String>,
    pub cpu_usage: Info<f64>,
    pub cpu_time: Info<Duration>,
    pub mem_usage: Info<f64>,
    pub virtual_mem_size: Info<u64>,
    pub physical_mem_size: Info<u64>,
    pub controlling_tty: Info<Option<String>>,
    pub start_time: Info<SystemTime>,
}

impl ProcessInfo {
    #[cfg(target_vendor = "apple")]
    pub const SIP_PREFIXES: &'static [&'static str] = &[
        "/bin",
        "/sbin",
        "/usr/bin",
        "/usr/sbin",
        "/usr/libexec",
        "/System",
        "/Library/Apple",
        "/private/var/db/com.apple.xpc.roleaccountd.staging",
    ];

    pub fn list_all() -> impl ParallelIterator<Item = (Pid, Self)> {
        let pids = Pid::all_active()
            .expect("couldn't list all PIDs")
            .collect::<Vec<_>>();
        pids.into_par_iter().filter_map(|pid| match pid.info() {
            Ok(info) => Some((pid, info)),
            Err(err) => {
                eprintln!("Couldn't get info for PID {pid}: {err}.");
                None
            }
        })
    }

    #[cfg(target_vendor = "apple")]
    pub fn is_sip_protected(&self) -> bool {
        ProcessInfo::SIP_PREFIXES.iter().any(|&prefix| {
            self.path
                .to_inner_option()
                .map_or(false, |path| path.as_bytes().starts_with(prefix.as_bytes()))
        })
    }
}
