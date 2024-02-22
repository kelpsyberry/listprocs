#![warn(clippy::all)]

mod cli;
mod ffi;
use ffi::{Pid, Uid};
mod utils;

use std::{cmp::Ordering, io};

#[derive(Clone, Debug)]
enum Info<T> {
    Unauthorized,
    Some(T),
}

impl Info<Option<String>> {
    fn to_str(&self) -> &str {
        match self {
            Info::Unauthorized => "<unauthorized>",
            Info::Some(None) => "<unknown>",
            Info::Some(Some(info)) => info,
        }
    }

    fn to_option(&self) -> Option<&str> {
        match self {
            Info::Some(None) | Info::Unauthorized => None,
            Info::Some(Some(info)) => Some(info),
        }
    }

    fn cmp_by(&self, other: &Self, compare: impl FnOnce(&str, &str) -> Ordering) -> Ordering {
        let kind = |info: &Self| match info {
            Info::Unauthorized => 0,
            Info::Some(None) => 1,
            Info::Some(Some(_)) => 2,
        };
        kind(self)
            .cmp(&kind(other))
            .then(compare(self.to_str(), other.to_str()))
    }
}

impl Info<String> {
    fn to_str(&self) -> &str {
        match self {
            Info::Unauthorized => "<unauthorized>",
            Info::Some(info) => info,
        }
    }

    fn to_option(&self) -> Option<&str> {
        match self {
            Info::Unauthorized => None,
            Info::Some(info) => Some(info),
        }
    }

    fn cmp_by(&self, other: &Self, compare: impl FnOnce(&str, &str) -> Ordering) -> Ordering {
        let kind = |info: &Info<String>| match info {
            Info::Unauthorized => 0,
            Info::Some(_) => 1,
        };
        kind(self)
            .cmp(&kind(other))
            .then(compare(self.to_str(), other.to_str()))
    }
}

#[derive(Debug)]
struct RunningProcessInfo {
    parent_pid: Pid,
    uid: Uid,
    username: String,
    path: Info<String>,
    cmd_line: Info<Option<String>>,
}

impl RunningProcessInfo {
    #[cfg(target_vendor = "apple")]
    fn is_sip_protected(&self) -> bool {
        ProcessInfo::SIP_PREFIXES.iter().any(|&prefix| {
            matches!(
                &self.path,
                Info::Some(path) if path.as_bytes().starts_with(prefix.as_bytes()))
        })
    }
}

#[derive(Debug)]
enum ProcessInfo {
    Defunct,
    Running(RunningProcessInfo),
}

impl ProcessInfo {
    #[cfg(target_vendor = "apple")]
    const SIP_PREFIXES: &'static [&'static str] = &[
        "/bin",
        "/sbin",
        "/usr/bin",
        "/usr/sbin",
        "/usr/libexec",
        "/System",
    ];

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

    fn path_str(&self) -> &str {
        match self {
            ProcessInfo::Defunct => "<defunct>",
            ProcessInfo::Running(info) => info.path.to_str(),
        }
    }

    fn cmd_line_str(&self) -> &str {
        match self {
            ProcessInfo::Defunct => "<defunct>",
            ProcessInfo::Running(info) => info.cmd_line.to_str(),
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
