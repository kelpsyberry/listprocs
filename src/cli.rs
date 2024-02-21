mod list;
use list::ListArgs;
mod tree;
use tree::TreeArgs;
mod user_filter;
use user_filter::UserFilter;

use crate::ffi::{self, Pid, Uid};
use clap::{
    builder::{ArgPredicate, StringValueParser, TypedValueParser},
    ArgAction, Parser,
};
use regex::{Regex, RegexBuilder};
use std::{borrow::Borrow, cmp::Ordering, io};

#[derive(Debug)]
struct RunningProcessInfo {
    parent_pid: Pid,
    uid: Uid,
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
    fn new(pid: Pid) -> Result<Self, io::Error> {
        let bsd_info = ffi::ProcBsdShortInfo::for_pid(pid)?;
        let path = ffi::path_for_pid(pid)?;
        let uid = bsd_info.uid;
        let username = ffi::username_for_uid(uid)?;
        let cmd_line = ffi::CmdLine::for_pid(pid)?;

        Ok(RunningProcessInfo {
            parent_pid: bsd_info.parent_pid as Pid,
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

    fn cmd_line_str(&self) -> &str {
        match &self.cmd_line {
            ffi::CmdLine::None => "<unknown>",
            ffi::CmdLine::Unauthorized => "<unauthorized>",
            ffi::CmdLine::Some(cmd_line) => cmd_line,
        }
    }
}

#[derive(Debug)]
enum ProcessInfo {
    Defunct,
    Running(RunningProcessInfo),
}

impl ProcessInfo {
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

struct ProcessFilter {
    regex: Option<Regex>,
    invert_regex: bool,
    user_ids: Vec<Uid>,
    usernames: Vec<String>,
    include_defunct: bool,
    include_sip: bool,
}

impl ProcessInfo {
    fn new(pid: Pid) -> Result<Self, io::Error> {
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

    fn list_all() -> impl Iterator<Item = (Pid, Self)> {
        ffi::all_pids()
            .expect("couldn't list all PIDs")
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
    }

    fn filter(_pid: Pid, info: &Self, filter: &ProcessFilter) -> bool {
        (match info {
            ProcessInfo::Defunct => filter.include_defunct,
            ProcessInfo::Running(info) => filter.include_sip || !info.is_sip_protected(),
        }) && {
            filter.usernames.is_empty()
                || match info {
                    ProcessInfo::Defunct => false,
                    ProcessInfo::Running(info) => filter.usernames.contains(&info.username),
                }
        } && {
            filter.user_ids.is_empty()
                || match info {
                    ProcessInfo::Defunct => false,
                    ProcessInfo::Running(info) => filter.user_ids.contains(&info.uid),
                }
        } && {
            filter.regex.as_ref().map_or(true, |regex| {
                filter.invert_regex
                    != match info {
                        ProcessInfo::Defunct => regex.is_match("<defunct>"),
                        ProcessInfo::Running(info) => {
                            regex.is_match(&info.path) || regex.is_match(info.cmd_line_str())
                        }
                    }
            })
        }
    }

    fn apply_filter<'a, P: Borrow<Pid>, I: Borrow<ProcessInfo>>(
        info: impl IntoIterator<Item = (P, I)> + 'a,
        filter: &'a ProcessFilter,
    ) -> impl Iterator<Item = (P, I)> + 'a {
        info.into_iter()
            .filter(|(pid, info)| Self::filter(*pid.borrow(), info.borrow(), filter))
    }
}

struct GlobalOptions {
    filter: ProcessFilter,
    use_box_drawing: bool,
    terminal_width: Option<usize>,
}

fn regex_parser() -> impl TypedValueParser {
    StringValueParser::new().try_map(|s| RegexBuilder::new(&s).case_insensitive(true).build())
}

fn user_filter_parser() -> impl TypedValueParser {
    user_filter::Parser
}

fn include_sip_long_help() -> String {
    format!(
        "Whether to include SIP-protected executables. Executables are considered SIP-protected \
         if they're in any of the following paths: {}.
Defaults to true if using a regex, and false otherwise.",
        SIP_PREFIXES.join(", ")
    )
}

#[derive(clap::Subcommand)]
enum Subcommand {
    Tree(TreeArgs),
}

#[derive(clap::Parser)]
/// A utility to list running processes and their info on macOS.
#[command(
    name = "listprocs",
    version,
    author,
    propagate_version = true,
    disable_help_subcommand = true,
    subcommand_negates_reqs = true,
    args_conflicts_with_subcommands = true
)]
struct Args {
    #[arg(global = true, value_name = "REGEX", value_parser(regex_parser()))]
    /// The regular expression to filter processes by (will be matched against each process's path
    /// and command line independently).
    regex: Option<Regex>,
    #[arg(
        global = true,
        action = ArgAction::Set,
        short,
        long,
        visible_alias = "invert",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false"
    )]
    /// Whether to filter regex matches out, instead of restricting the search to them.
    invert_matches: bool,
    #[arg(
        global = true,
        short,
        long = "user",
        value_name = "UID|USERNAME|'-'",
        value_parser(user_filter_parser()),
        allow_hyphen_values = true,
        require_equals = true,
        num_args = 0..,
        value_delimiter = ',',
        default_missing_value = "-",
    )]
    /// If present, only show processes belonging to the specified UIDs or usernames (a hyphen or
    /// no value will select the current UID); if unspecified processes won't be filtered by user.
    user_filter: Option<Vec<UserFilter>>,

    #[arg(
        global = true,
        action = ArgAction::Set,
        long = "defunct",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to include defunct processes.
    include_defunct: bool,
    #[arg(
        global = true,
        action = ArgAction::Set,
        long = "sip",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
        default_value_if("regex", ArgPredicate::IsPresent, Some("true")),
        long_help = include_sip_long_help(),
    )]
    /// Whether to include SIP-protected executables.
    include_sip: bool,
    #[arg(
        global = true,
        action = ArgAction::Set,
        long = "ascii",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to only use ASCII for output.
    use_ascii: bool,

    #[command(subcommand)]
    subcommand: Option<Subcommand>,
    #[command(flatten)]
    list_args: ListArgs,
}

pub fn main() {
    let args = Args::parse();

    let mut user_ids = Vec::new();
    let mut usernames = Vec::new();
    for filter in args.user_filter.into_iter().flatten() {
        match filter {
            UserFilter::Uid(uid) => user_ids.push(uid),
            UserFilter::Username(username) => usernames.push(username),
        }
    }

    let options = GlobalOptions {
        filter: ProcessFilter {
            regex: args.regex,
            invert_regex: args.invert_matches,
            user_ids,
            usernames,
            include_defunct: args.include_defunct,
            include_sip: args.include_sip,
        },
        use_box_drawing: !args.use_ascii,
        terminal_width: terminal_size::terminal_size().map(|size| size.0 .0 as usize),
    };

    match args.subcommand {
        Some(Subcommand::Tree(tree_args)) => tree::tree(options, tree_args),
        None => list::list(options, args.list_args),
    }
}
