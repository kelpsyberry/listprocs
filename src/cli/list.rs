use super::GlobalOptions;
use crate::{
    utils::{format_mem, table},
    Pid, ProcessInfo,
};
use chrono::{DateTime, Local};
use clap::builder::ArgAction;
use rayon::prelude::*;
use std::{cmp::Ordering, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColumnName {
    Pid,
    #[value(alias("ppid"))]
    ParentPid,
    Uid,
    #[value(name = "user", alias("username"))]
    Username,
    Path,
    #[value(name = "cmd", alias("cmd-line"))]
    CmdLine,
    #[value(name = "name", alias("comm-name"))]
    Name,
    AnyName,
    #[value(name = "cpu", alias("cpu-usage"))]
    CpuUsage,
    #[value(name = "mem", alias("mem-usage"))]
    MemUsage,
    #[value(name = "vm", alias("virt-mem"), alias("virtual-mem"), alias("vm-size"))]
    VirtualMemSize,
    #[value(
        name = "phys",
        alias("phys-mem"),
        alias("physical-mem"),
        alias("rss"),
        alias("resident"),
        alias("resident-size")
    )]
    PhysicalMemSize,
    Tty,
    #[value(name = "start", alias("start-time"))]
    StartTime,
    #[value(name = "time", alias("cpu-time"))]
    CpuTime,
}

#[derive(clap::Parser)]
pub struct ListArgs {
    #[arg(
        short,
        long,
        value_name = "COLUMN",
        require_equals = true,
        num_args = 1..,
        value_delimiter = ',',
        default_value = "pid,user,path,cmd",
    )]
    /// Which columns to display.
    cols: Vec<ColumnName>,
    #[arg(
        short,
        long,
        value_name = "COLUMN",
        require_equals = true,
        num_args = 1..,
        value_delimiter = ',',
        default_value = "pid",
    )]
    /// Which column(s) to sort by, in order of decreasing priority.
    sort: Vec<ColumnName>,
    #[arg(
        global = true,
        action = ArgAction::Set,
        short,
        long,
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to produce plain output, without any table borders.
    plain: bool,
    #[arg(
        global = true,
        action = ArgAction::Set,
        long = "ps",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to produce ps-compatible output for data.
    ps_compat: bool,
}

pub fn list(options: GlobalOptions, args: ListArgs) {
    fn ord_ne(ordering: Ordering) -> Option<Ordering> {
        match ordering {
            Ordering::Equal => None,
            _ => Some(ordering),
        }
    }

    let mut processes_info =
        ProcessInfo::par_apply_filter(ProcessInfo::list_all(), &options.filter).collect::<Vec<_>>();
    if !args.sort.is_empty() {
        processes_info.sort_by(|(a_pid, a_info), (b_pid, b_info)| {
            args.sort
                .iter()
                .find_map(|column| {
                    ord_ne(match column {
                        ColumnName::Pid => a_pid.cmp(b_pid),
                        ColumnName::ParentPid => a_info.parent_pid.cmp(&b_info.parent_pid),
                        ColumnName::Uid => a_info.uid.cmp(&b_info.uid),
                        ColumnName::Username => a_info.username.cmp(&b_info.username),
                        ColumnName::Path => a_info.path.cmp(&b_info.path),
                        ColumnName::CmdLine => a_info.cmd_line.cmp(&b_info.cmd_line),
                        ColumnName::Name => a_info.name.cmp(&b_info.name),
                        ColumnName::AnyName => a_info
                            .cmd_line
                            .cmp(&b_info.cmd_line)
                            .then_with(|| a_info.name.cmp(&b_info.name))
                            .then_with(|| a_info.path.cmp(&b_info.path))
                            .then_with(|| (!a_info.is_defunct).cmp(&(!b_info.is_defunct))),
                        ColumnName::CpuUsage => a_info
                            .cpu_usage
                            .partial_cmp(&b_info.cpu_usage)
                            .unwrap_or(Ordering::Equal),
                        ColumnName::MemUsage => a_info
                            .mem_usage
                            .partial_cmp(&b_info.mem_usage)
                            .unwrap_or(Ordering::Equal),
                        ColumnName::CpuTime => a_info.cpu_time.cmp(&b_info.cpu_time),
                        ColumnName::VirtualMemSize => {
                            a_info.virtual_mem_size.cmp(&b_info.virtual_mem_size)
                        }
                        ColumnName::PhysicalMemSize => {
                            a_info.physical_mem_size.cmp(&b_info.physical_mem_size)
                        }
                        ColumnName::Tty => a_info.controlling_tty.cmp(&b_info.controlling_tty),
                        ColumnName::StartTime => a_info.start_time.cmp(&b_info.start_time),
                    })
                })
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut columns = args
        .cols
        .into_iter()
        .map(|name| match name {
            ColumnName::Pid => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "PID",
                Box::new(|(pid, _)| pid.to_string().into()),
            )
            .calc_width(Box::new(|(pid, _)| pid.raw().max(1).ilog10() as usize + 1))
            .h_padding(Some(1))
            .build(),

            ColumnName::ParentPid => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "PPID" } else { "Parent" },
                Box::new(|(_, info)| match info.parent_pid.to_option() {
                    None => "-".into(),
                    Some(parent_pid) => parent_pid.to_string().into(),
                }),
            )
            .calc_width(Box::new(|(_, info)| match info.parent_pid.to_option() {
                None => 1,
                Some(parent_pid) => parent_pid.raw().max(1).ilog10() as usize + 1,
            }))
            .h_padding(Some(1))
            .build(),

            ColumnName::Uid => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "UID",
                Box::new(|(_, info)| match info.uid.to_option() {
                    None => "-".into(),
                    Some(uid) => uid.to_string().into(),
                }),
            )
            .calc_width(Box::new(|(_, info)| match info.uid.to_option() {
                None => 1,
                Some(uid) => uid.raw().max(1).ilog10() as usize + 1,
            }))
            .h_padding(Some(1))
            .build(),

            ColumnName::Username => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "USER" } else { "User" },
                Box::new(|(_, info)| {
                    match info.username.to_option() {
                        None => "-",
                        Some(username) => username,
                    }
                    .into()
                }),
            )
            .h_padding(Some(1))
            .build(),

            ColumnName::Path => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "PATH" } else { "Path" },
                Box::new(|(_, info)| info.path.to_str().into()),
            )
            .can_shrink(true)
            .build(),

            ColumnName::CmdLine => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat {
                    "COMMAND"
                } else {
                    "Command line"
                },
                Box::new(|(_, info)| info.cmd_line.to_str().into()),
            )
            .can_shrink(true)
            .build(),

            ColumnName::Name => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "NAME" } else { "Name" },
                Box::new(|(_, info)| info.name.to_str().into()),
            )
            .can_shrink(true)
            .build(),

            ColumnName::AnyName => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat {
                    "CMD/[NAME]/<PATH>"
                } else {
                    "Cmd/[Name]/<Path>"
                },
                Box::new(|(_, info)| {
                    info.cmd_line
                        .to_inner_option()
                        .map(Into::into)
                        .or_else(|| {
                            info.path
                                .to_inner_option()
                                .map(|path| format!("<{path}>").into())
                        })
                        .or_else(|| {
                            info.name.to_option().map(|name| {
                                let mut result = format!("[{}]", name);
                                if info.is_defunct {
                                    result.push_str(" <defunct>");
                                }
                                result.into()
                            })
                        })
                        .unwrap_or_else(|| info.name.to_str().into())
                }),
            )
            .can_shrink(true)
            .build(),

            ColumnName::CpuUsage => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "%CPU" } else { "CPU" },
                Box::new(|(_, info)| match info.cpu_usage.to_option() {
                    None => "-".into(),
                    Some(cpu_usage) => if args.ps_compat {
                        format!(
                            "{:.precision$}",
                            cpu_usage * 100.0,
                            precision = if args.ps_compat { 1 } else { 2 }
                        )
                    } else {
                        format!(
                            "{:.precision$}%",
                            cpu_usage * 100.0,
                            precision = if args.ps_compat { 1 } else { 2 }
                        )
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1))
            .build(),

            ColumnName::CpuTime => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "TIME" } else { "CPU time" },
                Box::new(|(_, info)| match info.cpu_time.to_option() {
                    None => "-".into(),
                    Some(cpu_time) => {
                        let secs = cpu_time.as_secs_f64();
                        format!("{:02.0}:{:05.2}", (secs / 60.0).floor(), secs % 60.0)
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1))
            .build(),

            ColumnName::MemUsage => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "%MEM" } else { "Mem" },
                Box::new(|(_, info)| match info.mem_usage.to_option() {
                    None => "-".into(),
                    Some(mem_usage) => if args.ps_compat {
                        format!(
                            "{:.precision$}",
                            mem_usage * 100.0,
                            precision = if args.ps_compat { 1 } else { 2 }
                        )
                    } else {
                        format!(
                            "{:.precision$}%",
                            mem_usage * 100.0,
                            precision = if args.ps_compat { 1 } else { 2 }
                        )
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1))
            .build(),

            ColumnName::VirtualMemSize => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "VSZ" } else { "Virt mem" },
                Box::new(|(_, info)| match info.virtual_mem_size.to_option() {
                    None => "-".into(),
                    Some(vm_size) => if args.ps_compat {
                        (*vm_size >> 10).to_string()
                    } else {
                        format_mem(*vm_size)
                    }
                    .into(),
                }),
            )
            .calc_width(Box::new(|(_, info)| {
                match info.virtual_mem_size.to_option() {
                    None => 1,
                    Some(vm_size) => {
                        if args.ps_compat {
                            (*vm_size >> 10).max(1).ilog10() as usize + 1
                        } else {
                            format_mem(*vm_size).len()
                        }
                    }
                }
            }))
            .h_padding(Some(1))
            .build(),

            ColumnName::PhysicalMemSize => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "RSS" } else { "Phys mem" },
                Box::new(|(_, info)| match info.physical_mem_size.to_option() {
                    None => "-".into(),
                    Some(phys_size) => if args.ps_compat {
                        (*phys_size >> 10).to_string()
                    } else {
                        format_mem(*phys_size)
                    }
                    .into(),
                }),
            )
            .calc_width(Box::new(|(_, info)| {
                match info.physical_mem_size.to_option() {
                    None => 1,
                    Some(phys_size) => {
                        if args.ps_compat {
                            (*phys_size >> 10).max(1).ilog10() as usize + 1
                        } else {
                            format_mem(*phys_size).len()
                        }
                    }
                }
            }))
            .h_padding(Some(1))
            .build(),

            ColumnName::Tty => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "TT" } else { "TTY" },
                Box::new(|(_, info)| match info.controlling_tty.to_option() {
                    None => "-".into(),
                    Some(None) => if args.ps_compat { "??" } else { "?" }.into(),
                    Some(Some(controlling_tty)) => controlling_tty.into(),
                }),
            )
            .h_padding(Some(1))
            .build(),

            ColumnName::StartTime => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                if args.ps_compat { "STARTED" } else { "Start" },
                Box::new(|(_, info)| match info.start_time.to_option() {
                    None => "-".into(),
                    Some(start_time) => {
                        let elapsed = start_time.elapsed().unwrap_or(Duration::ZERO);
                        let use_am_pm = true; // TODO
                        let format = if args.ps_compat {
                            if elapsed.as_secs() < 24 * 3600 {
                                if use_am_pm {
                                    "%l:%M%p"
                                } else {
                                    "%k:%M"
                                }
                            } else if elapsed.as_secs() < 7 * 24 * 3600 {
                                if use_am_pm {
                                    "%a%I%p"
                                } else {
                                    "%a%H"
                                }
                            } else {
                                "%e%b%y"
                            }
                        } else if elapsed.as_secs() < 24 * 3600 {
                            if use_am_pm {
                                "%-l:%M %p"
                            } else {
                                "%k:%M"
                            }
                        } else if elapsed.as_secs() < 7 * 24 * 3600 {
                            if use_am_pm {
                                "%a %-l:%M %p"
                            } else {
                                "%a %k:%M"
                            }
                        } else if use_am_pm {
                            "%e %b %y %-l:%M %p"
                        } else {
                            "%e %b %y %k:%M"
                        };
                        DateTime::<Local>::from(*start_time)
                            .format(format)
                            .to_string()
                            .into()
                    }
                }),
            )
            .h_padding(Some(1))
            .build(),
        })
        .collect::<Vec<_>>();

    print!(
        "{}",
        table::Builder::new()
            .style(if args.plain {
                table::Style::None
            } else if options.use_box_drawing {
                table::Style::BoxDrawing
            } else {
                table::Style::Ascii
            })
            .h_padding(2)
            .build(&mut columns, &processes_info, options.terminal_width)
    );
}
