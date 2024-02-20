#![warn(clippy::all)]

mod ffi;
mod utils;
use utils::table;

use libc::{pid_t, uid_t};
use regex::{Regex, RegexBuilder};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap},
    convert::Infallible,
    io, iter,
};

#[derive(Debug)]
struct RunningProcessInfo {
    parent_pid: pid_t,
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
            parent_pid: bsd_info.parent_pid as pid_t,
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

struct ProcessFilter<'a> {
    regex: Option<Regex>,
    invert_regex: bool,
    user_ids: Vec<uid_t>,
    usernames: Vec<&'a String>,
    include_defunct: bool,
    include_sip: bool,
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

    fn list_all() -> impl Iterator<Item = (pid_t, Self)> {
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

    fn filter(_pid: pid_t, info: &Self, filter: &ProcessFilter) -> bool {
        (match info {
            ProcessInfo::Defunct => filter.include_defunct,
            ProcessInfo::Running(info) => filter.include_sip || !info.is_sip_protected(),
        }) && {
            filter.usernames.is_empty()
                || match info {
                    ProcessInfo::Defunct => false,
                    ProcessInfo::Running(info) => filter.usernames.contains(&&info.username),
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
                            regex.is_match(&info.path)
                                || regex.is_match(match &info.cmd_line {
                                    ffi::CmdLine::None => "<unknown>",
                                    ffi::CmdLine::Unauthorized => "<unauthorized>",
                                    ffi::CmdLine::Some(cmd_line) => cmd_line,
                                })
                        }
                    }
            })
        }
    }

    fn apply_filter<'a, P: Borrow<pid_t>, I: Borrow<ProcessInfo>>(
        info: impl IntoIterator<Item = (P, I)> + 'a,
        filter: &'a ProcessFilter,
    ) -> impl Iterator<Item = (P, I)> + 'a {
        info.into_iter()
            .filter(|(pid, info)| Self::filter(*pid.borrow(), info.borrow(), filter))
    }
}

#[derive(Clone)]
enum UserFilter {
    Uid(uid_t),
    Username(String),
}

struct GlobalOptions<'a> {
    filter: ProcessFilter<'a>,
    use_box_drawing: bool,
    terminal_width: Option<usize>,
}

fn tree(options: GlobalOptions, args: &clap::ArgMatches) {
    #[derive(Debug)]
    struct Node(BTreeMap<pid_t, Node>);

    fn create_tree<'a>(
        processes_info: impl IntoIterator<Item = (&'a pid_t, &'a ProcessInfo)>,
        full_processes_info: &HashMap<pid_t, ProcessInfo>,
    ) -> Node {
        let mut root = Node(BTreeMap::default());
        let mut cur_path = Vec::new();

        for (pid, info) in processes_info {
            cur_path.extend(iter::successors(
                Some((*pid, info)),
                |(_, info)| match info {
                    ProcessInfo::Defunct => None,
                    ProcessInfo::Running(info) => full_processes_info
                        .get(&info.parent_pid)
                        .map(|parent_info| (info.parent_pid, parent_info)),
                },
            ));
            let mut cur_root = &mut root;
            while let Some((pid, _)) = cur_path.pop() {
                cur_root = cur_root.0.entry(pid).or_insert(Node(BTreeMap::default()));
            }
        }

        root
    }

    fn print(
        Node(children): &Node,
        borders: &mut String,
        processes_info: &HashMap<pid_t, ProcessInfo>,
        options: &GlobalOptions,
    ) {
        for (i, (pid, child_children)) in children.iter().enumerate() {
            let info = &processes_info[pid];
            let mut name = match info {
                ProcessInfo::Defunct => "<defunct>",
                ProcessInfo::Running(info) => match &info.cmd_line {
                    ffi::CmdLine::None | ffi::CmdLine::Unauthorized => &info.path,
                    ffi::CmdLine::Some(cmd_line) => cmd_line,
                },
            }
            .to_string();

            if let Some(max_len) = options.terminal_width.map(|width| {
                width
                    - (borders.chars().count() + 2 + 1 + ((*pid).max(1).ilog10() as usize + 1) + 1)
            }) {
                utils::truncate_string(&mut name, max_len);
            }

            let is_first = i == 0;
            let is_last = i == children.len() - 1;

            let (border, h_line) = if options.use_box_drawing {
                (
                    match (is_first && borders.is_empty(), is_last) {
                        (false, false) => '├',
                        (false, true) => '└',
                        (true, false) => '┌',
                        (true, true) => '─',
                    },
                    if child_children.0.is_empty() {
                        '─'
                    } else {
                        '┬'
                    },
                )
            } else {
                (
                    match (is_first && borders.is_empty(), is_last) {
                        (false, false) => '|',
                        (false, true) => '\\',
                        (true, false) => '/',
                        (true, true) => '-',
                    },
                    '-',
                )
            };
            println!("{borders}{border}{h_line} {pid} {name}");

            if is_last {
                borders.push(' ');
            } else {
                borders.push(['|', '│'][options.use_box_drawing as usize]);
            }
            print(child_children, borders, processes_info, options);
            borders.pop();
        }
    }

    let include_ancestors = *args.get_one::<bool>("ancestors").unwrap();

    let processes_info_iter = ProcessInfo::list_all();
    let (root, processes_info) = if include_ancestors {
        let full_processes_info = processes_info_iter.collect::<HashMap<_, _>>();
        (
            create_tree(
                ProcessInfo::apply_filter(full_processes_info.iter(), &options.filter),
                &full_processes_info,
            ),
            full_processes_info,
        )
    } else {
        let processes_info = ProcessInfo::apply_filter(processes_info_iter, &options.filter)
            .collect::<HashMap<_, _>>();
        (
            create_tree(&processes_info, &processes_info),
            processes_info,
        )
    };

    print(&root, &mut String::new(), &processes_info, &options);
}

fn list(options: GlobalOptions, args: &clap::ArgMatches) {
    let enabled_cols = args.get_many::<String>("cols").unwrap().collect::<Vec<_>>();

    let processes_info =
        ProcessInfo::apply_filter(ProcessInfo::list_all(), &options.filter).collect::<Vec<_>>();

    let mut columns = enabled_cols
        .into_iter()
        .map(|name| match name.as_str() {
            "pid" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "PID",
                Box::new(|(pid, _)| pid.to_string().into()),
            )
            .calc_width(Box::new(|(pid, _)| (*pid).max(1).ilog10() as usize + 1))
            .h_padding(Some(1))
            .build(),
            "parent-pid" => table::ColumnBuilder::<(pid_t, ProcessInfo)>::new(
                "Parent PID",
                Box::new(|(_, info)| match info {
                    ProcessInfo::Defunct => "-".into(),
                    ProcessInfo::Running(info) => info.parent_pid.to_string().into(),
                }),
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
            .can_shrink(true)
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
            .can_shrink(true)
            .build(),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();

    println!(
        "{}",
        table::Builder::new()
            .use_box_drawing(options.use_box_drawing)
            .h_padding(2)
            .build(&mut columns, &processes_info, options.terminal_width)
    );
}

fn main() {
    let args = clap::Command::new("listprocs")
        .about("A utility to list running processes and their info on macOS")
        .version("0.0.0")
        .propagate_version(true)
        .disable_help_subcommand(true)
        .subcommand_negates_reqs(true)
        .args_conflicts_with_subcommands(true)
        .arg(
            clap::Arg::new("regex")
                .global(true)
                .value_name("REGEX")
                .help(
                    "The regular expression to filter processes by (will be matched against each \
                     process's path and command line independently)",
                ),
        )
        .arg(
            clap::Arg::new("invert-matches")
                .global(true)
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
            clap::Arg::new("user")
                .global(true)
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
            clap::Arg::new("defunct")
                .global(true)
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
                .global(true)
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
                .global(true)
                .long("ascii")
                .value_name("BOOL")
                .value_parser(clap::value_parser!(bool))
                .require_equals(true)
                .num_args(0..2)
                .default_missing_value("true")
                .default_value("false")
                .help("Only use ASCII for output"),
        )
        .subcommand(
            clap::Command::new("tree").arg(
                clap::Arg::new("ancestors")
                    .global(true)
                    .short('a')
                    .long("ancestors")
                    .value_name("BOOL")
                    .value_parser(clap::value_parser!(bool))
                    .require_equals(true)
                    .num_args(0..2)
                    .default_missing_value("true")
                    .default_value("false")
                    .help("Always show ancestors, even if filtered out"),
            ),
        )
        .arg(
            clap::Arg::new("cols")
                .short('c')
                .long("cols")
                .value_name("COLUMN")
                .value_parser(["pid", "parent-pid", "user", "path", "cmd"])
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

    let mut user_ids = Vec::new();
    let mut usernames = Vec::new();
    for filter in args.get_many::<UserFilter>("user").into_iter().flatten() {
        match filter {
            UserFilter::Uid(uid) => user_ids.push(*uid),
            UserFilter::Username(username) => usernames.push(username),
        }
    }

    let include_defunct = *args.get_one::<bool>("defunct").unwrap();
    let include_sip = *args.get_one::<bool>("sip").unwrap();
    let use_box_drawing = !*args.get_one::<bool>("ascii").unwrap();

    let options = GlobalOptions {
        filter: ProcessFilter {
            regex,
            invert_regex,
            user_ids,
            usernames,
            include_defunct,
            include_sip,
        },
        use_box_drawing,
        terminal_width: terminal_size::terminal_size().map(|size| size.0 .0 as usize),
    };

    match args.subcommand() {
        Some(("tree", args)) => tree(options, args),
        None => list(options, &args),
        _ => unreachable!(),
    }
}
