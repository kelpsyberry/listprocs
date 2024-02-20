mod ffi;
mod utils;
use utils::table;

use libc::{pid_t, uid_t};
use regex::RegexBuilder;
use std::{convert::Infallible, io};

#[derive(Debug)]
struct RunningProcessInfo {
    uid: uid_t,
    username: String,
    path: String,
    cmd_line: ffi::CmdLine<String>,
}

static SIP_PREFIXES: &[&str] = &[
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/usr/libexec",
    "/System",
];

impl RunningProcessInfo {
    fn new(pid: pid_t) -> Result<Self, io::Error> {
        let bsd_info = ffi::ProcBsdShortInfo::for_pid(pid)?;
        let path = ffi::path_for_pid(pid)?;
        let uid = bsd_info.uid;
        let username = ffi::username_for_uid(uid)?;
        let cmd_line = ffi::CmdLine::for_pid(pid)?;

        Ok(RunningProcessInfo {
            uid,
            username: username.to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            cmd_line: match cmd_line {
                ffi::CmdLine::None => ffi::CmdLine::None,
                ffi::CmdLine::Unauthorized => ffi::CmdLine::Unauthorized,
                ffi::CmdLine::Some(cmd_line) => {
                    ffi::CmdLine::Some(cmd_line.to_string_lossy().into_owned())
                }
            },
        })
    }

    fn is_sip_protected(&self) -> bool {
        SIP_PREFIXES
            .iter()
            .any(|&prefix| self.path.as_bytes().starts_with(prefix.as_bytes()))
    }
}

#[derive(Debug)]
enum ProcessInfo {
    Defunct,
    Running(RunningProcessInfo),
}

impl ProcessInfo {
    fn new(pid: pid_t) -> Result<Self, io::Error> {
        match RunningProcessInfo::new(pid) {
            Ok(info) => Ok(ProcessInfo::Running(info)),
            Err(err) => {
                if err.raw_os_error() == Some(3) {
                    Ok(ProcessInfo::Defunct)
                } else {
                    Err(err)
                }
            }
        }
    }
}

#[derive(Clone)]
enum UserFilter {
    Uid(uid_t),
    Username(String),
}

fn main() {
    let args = clap::Command::new("listprocs")
        .about("A utility to list running processes and their info on macOS")
        .version("0.0.0")
        .arg(clap::Arg::new("regex").value_name("REGEX").help(
            "The regular expression to filter processes by (will be matched against each \
             process's path and command line independently)",
        ))
        .arg(
            clap::Arg::new("user")
                .short('u')
                .long("user")
                .value_name("UID|USERNAME|'-'")
                .value_parser(|value: &str| -> Result<UserFilter, Infallible> {
                    if value == "-" {
                        Ok(UserFilter::Uid(ffi::current_uid()))
                    } else if let Ok(uid) = value.parse::<uid_t>() {
                        Ok(UserFilter::Uid(uid))
                    } else {
                        Ok(UserFilter::Username(value.to_string()))
                    }
                })
                .allow_hyphen_values(true)
                .require_equals(true)
                .num_args(0..)
                .value_delimiter(',')
                .default_missing_value("-")
                .help(
                    "Only show processes belonging to the specified UIDs or usernames (a hyphen \
                     will select the current UID)",
                ),
        )
        .arg(
            clap::Arg::new("invert-matches")
                .short('i')
                .long("invert-matches")
                .visible_alias("invert")
                .value_name("BOOL")
                .value_parser(clap::value_parser!(bool))
                .require_equals(true)
                .num_args(0..2)
                .default_missing_value("true")
                .default_value("false")
                .help("Invert regex matches"),
        )
        .arg(
            clap::Arg::new("defunct")
                .long("defunct")
                .value_name("BOOL")
                .value_parser(clap::value_parser!(bool))
                .require_equals(true)
                .num_args(0..2)
                .default_missing_value("true")
                .default_value("false")
                .help("Include defunct processes"),
        )
        .arg(
            clap::Arg::new("sip")
                .short('s')
                .long("sip")
                .value_name("BOOL")
                .value_parser(clap::value_parser!(bool))
                .require_equals(true)
                .num_args(0..2)
                .default_missing_value("true")
                .default_value("false")
                .default_value_if(
                    "regex",
                    clap::builder::ArgPredicate::IsPresent,
                    Some("true"),
                )
                .help("Include SIP-protected executables")
                .long_help(format!(
                    "Include SIP-protected executables. Executables are considered SIP-protected \
                     if they're in any of the following paths: {}.
Defaults to true if using a regex, and false otherwise.",
                    SIP_PREFIXES.join(", ")
                )),
        )
        .arg(
            clap::Arg::new("ascii")
                .long("ascii")
                .value_name("BOOL")
                .value_parser(clap::value_parser!(bool))
                .require_equals(true)
                .num_args(0..2)
                .default_missing_value("true")
                .default_value("false")
                .help("Only use ASCII for output"),
        )
        .arg(
            clap::Arg::new("cols")
                .short('c')
                .long("cols")
                .value_name("COLUMN")
                .value_parser(["pid", "user", "path", "cmd"])
                .require_equals(true)
                .num_args(1..)
                .value_delimiter(',')
                .default_value("pid,user,path,cmd")
                .help("Which columns to display"),
        )
        .get_matches();

    let regex = args.get_one::<String>("regex").map(|pattern| {
        RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .expect("invalid regex")
    });
    let invert_regex = *args.get_one::<bool>("invert-matches").unwrap();
    let filter_non_sip = !*args.get_one::<bool>("sip").unwrap();
    let filter_defunct = !*args.get_one::<bool>("defunct").unwrap();

    let use_box_drawing = !*args.get_one::<bool>("ascii").unwrap();

    let enabled_cols = args.get_many::<String>("cols").unwrap().collect::<Vec<_>>();

    let mut user_ids = Vec::new();
    let mut usernames = Vec::new();
    for filter in args.get_many::<UserFilter>("user").into_iter().flatten() {
        match filter {
            UserFilter::Uid(uid) => user_ids.push(*uid),
            UserFilter::Username(username) => usernames.push(username),
        }
    }

    let mut all_pids = ffi::all_pids().expect("couldn't list all PIDs");
    all_pids.sort_unstable();

    let processes_info = all_pids
        .into_iter()
        .filter(|&pid| pid != 0)
        .filter_map(|pid| match ProcessInfo::new(pid) {
            Ok(info) => Some((pid, info)),
            Err(err) => match err.kind() {
                io::ErrorKind::PermissionDenied => None,
                _ => {
                    eprintln!("Couldn't get info for PID {pid}: {} {err}.", err.kind());
                    None
                }
            },
        })
        .filter(|(_, info)| match info {
            ProcessInfo::Defunct => !filter_defunct,
            ProcessInfo::Running(info) => !filter_non_sip || !info.is_sip_protected(),
        })
        .filter(|(_, info)| {
            usernames.is_empty()
                || match info {
                    ProcessInfo::Defunct => false,
                    ProcessInfo::Running(info) => usernames.contains(&&info.username),
                }
        })
        .filter(|(_, info)| {
            user_ids.is_empty()
                || match info {
                    ProcessInfo::Defunct => false,
                    ProcessInfo::Running(info) => user_ids.contains(&info.uid),
                }
        })
        .filter(|(_, info)| {
            regex.as_ref().map_or(true, |regex| {
                invert_regex
                    != match info {
                        ProcessInfo::Defunct => regex.is_match("<defunct>"),
                        ProcessInfo::Running(info) => {
                            regex.is_match(&info.path)
                                || regex.is_match(match &info.cmd_line {
                                    ffi::CmdLine::None => "<unknown>",
                                    ffi::CmdLine::Unauthorized => "<unauthorized>",
                                    ffi::CmdLine::Some(cmd_line) => cmd_line,
                                })
                        }
                    }
            })
        })
        .collect::<Vec<_>>();

    let mut columns = enabled_cols
        .into_iter()
        .map(|name| match name.as_str() {
            "pid" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "PID",
                Box::new(|(pid, _)| format!("{pid}").into()),
            )
            .calc_width(Box::new(|(pid, _)| (*pid).max(1).ilog10() as usize + 1))
            .h_padding(Some(1))
            .build(),
            "user" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "User",
                Box::new(|(_, info)| {
                    match info {
                        ProcessInfo::Defunct => "-",
                        ProcessInfo::Running(info) => &info.username,
                    }
                    .into()
                }),
            )
            .h_padding(Some(1))
            .build(),
            "path" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "Path",
                Box::new(|(_, info)| {
                    match info {
                        ProcessInfo::Defunct => "<defunct>",
                        ProcessInfo::Running(info) => &info.path,
                    }
                    .into()
                }),
            )
            .max_width(Some(100))
            .build(),
            "cmd" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "Command line",
                Box::new(|(_, info)| {
                    match info {
                        ProcessInfo::Defunct => "<defunct>",
                        ProcessInfo::Running(info) => match &info.cmd_line {
                            ffi::CmdLine::None => "<unknown>",
                            ffi::CmdLine::Unauthorized => "<unauthorized>",
                            ffi::CmdLine::Some(cmd_line) => cmd_line,
                        },
                    }
                    .into()
                }),
            )
            .max_width(Some(100))
            .build(),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        table::Builder::new()
            .use_box_drawing(use_box_drawing)
            .h_padding(2)
            .build(&mut columns, &processes_info)
    );
}
