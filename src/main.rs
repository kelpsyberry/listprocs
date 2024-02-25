#![warn(clippy::all)]

mod cli;
mod ffi;
use ffi::{Pid, Uid};
mod utils;

use rayon::prelude::*;
use std::time::{Duration, SystemTime};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Info<T> {
    Defunct,
    Unauthorized,
    Some(T),
}

impl<T> Info<T> {
    fn to_option(&self) -> Option<&T> {
        match self {
            Info::Defunct | Info::Unauthorized => None,
            Info::Some(info) => Some(info),
        }
    }

    fn map<U>(self, f: impl FnOnce(T) -> U) -> Info<U> {
        match self {
            Info::Defunct => Info::Defunct,
            Info::Unauthorized => Info::Unauthorized,
            Info::Some(info) => Info::Some(f(info)),
        }
    }
}

impl<T> Info<Option<T>> {
    fn to_inner_option(&self) -> Option<&T> {
        match self {
            Info::Defunct | Info::Unauthorized => None,
            Info::Some(info) => info.as_ref(),
        }
    }
}

impl Info<Option<String>> {
    fn to_str(&self) -> &str {
        match self {
            Info::Defunct => "<defunct>",
            Info::Unauthorized => "<unauthorized>",
            Info::Some(None) => "<unknown>",
            Info::Some(Some(info)) => info,
        }
    }
}

impl Info<String> {
    fn to_str(&self) -> &str {
        match self {
            Info::Defunct => "<defunct>",
            Info::Unauthorized => "<unauthorized>",
            Info::Some(info) => info,
        }
    }
}

#[derive(Debug)]
struct ProcessInfo {
    is_defunct: bool,
    parent_pid: Info<Pid>,
    uid: Info<Uid>,
    username: Info<String>,
    path: Info<Option<String>>,
    cmd_line: Info<Option<String>>,
    name: Info<String>,
    cpu_usage: Info<f64>,
    cpu_time: Info<Duration>,
    mem_usage: Info<f64>,
    virtual_mem_size: Info<u64>,
    physical_mem_size: Info<u64>,
    controlling_tty: Info<Option<String>>,
    start_time: Info<SystemTime>,
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

    fn list_all() -> impl ParallelIterator<Item = (Pid, Self)> {
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
    fn is_sip_protected(&self) -> bool {
        ProcessInfo::SIP_PREFIXES.iter().any(|&prefix| {
            self.path
                .to_inner_option()
                .map_or(false, |path| path.as_bytes().starts_with(prefix.as_bytes()))
        })
    }
}

fn main() {
    cli::main();
}
