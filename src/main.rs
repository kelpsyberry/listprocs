#![warn(clippy::all)]

mod cli;
mod ffi;
use ffi::{Pid, Uid};
mod utils;

use std::{cmp::Ordering, io};

#[cfg(target_vendor = "apple")]
static SIP_PREFIXES: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/usr/libexec",
    "/System",
];

#[derive(Clone, Debug)]
enum CmdLine<S> {
    None,
    Unauthorized,
    Some(S),
}

#[derive(Debug)]
struct RunningProcessInfo {
    parent_pid: Pid,
    uid: Uid,
    username: String,
    path: String,
    cmd_line: CmdLine<String>,
}

impl RunningProcessInfo {
    #[cfg(target_vendor = "apple")]
    fn is_sip_protected(&self) -> bool {
        SIP_PREFIXES
            .iter()
            .any(|&prefix| self.path.as_bytes().starts_with(prefix.as_bytes()))
    }

    fn cmd_line_str(&self) -> &str {
        match &self.cmd_line {
            CmdLine::None => "<unknown>",
            CmdLine::Unauthorized => "<unauthorized>",
            CmdLine::Some(cmd_line) => cmd_line,
        }
    }
}

#[derive(Debug)]
enum ProcessInfo {
    Defunct,
    Running(RunningProcessInfo),
}

impl ProcessInfo {
    fn list_all() -> impl Iterator<Item = (Pid, Self)> {
        Pid::all_active()
            .expect("couldn't list all PIDs")
            .filter_map(|pid| match pid.info() {
                Ok(info) => Some((pid, info)),
                Err(err) => match err.kind() {
                    io::ErrorKind::PermissionDenied => None,
                    _ => {
                        eprintln!("Couldn't get info for PID {pid}: {err}.");
                        None
                    }
                },
            })
    }

    fn cmd_line_str(&self) -> &str {
        match self {
            ProcessInfo::Defunct => "<defunct>",
            ProcessInfo::Running(info) => info.cmd_line_str(),
        }
    }

    fn cmp_by(
        &self,
        other: &Self,
        compare: impl FnOnce(&RunningProcessInfo, &RunningProcessInfo) -> Ordering,
    ) -> Ordering {
        match (self, other) {
            (ProcessInfo::Running(a), ProcessInfo::Running(b)) => compare(a, b),
            (ProcessInfo::Defunct, ProcessInfo::Running(_)) => Ordering::Less,
            (ProcessInfo::Running(_), ProcessInfo::Defunct) => Ordering::Greater,
            (ProcessInfo::Defunct, ProcessInfo::Defunct) => Ordering::Equal,
        }
    }
}

fn main() {
    cli::main();
}
