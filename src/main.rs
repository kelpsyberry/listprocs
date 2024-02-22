#![warn(clippy::all)]

mod cli;
mod ffi;
use ffi::{Pid, Uid};
mod utils;

use std::io;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Info<T> {
    Unauthorized,
    Defunct,
    Some(T),
}

impl<T> Info<T> {
    fn to_option(&self) -> Option<&T> {
        match self {
            Info::Defunct | Info::Unauthorized => None,
            Info::Some(info) => Some(info),
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
    cmd_line: Info<Option<String>>,
    path: Info<String>,
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

    #[cfg(target_vendor = "apple")]
    fn is_sip_protected(&self) -> bool {
        ProcessInfo::SIP_PREFIXES.iter().any(|&prefix| {
            self.path
                .to_option()
                .map_or(false, |path| path.as_bytes().starts_with(prefix.as_bytes()))
        })
    }
}

fn main() {
    cli::main();
}
