use super::GlobalOptions;
use crate::{
    utils::{format_mem, table},
    Pid, ProcessInfo,
};
use chrono::{DateTime, Local};
use clap::builder::ArgAction;
use rayon::prelude::*;
use std::{borrow::Cow, cmp::Ordering, time::Duration};

pub type CalcWidth<'a, T> = Box<dyn Fn(&T) -> usize + 'a>;
pub type CalcValue<'a, T> = Box<dyn Fn(&T) -> Cow<str> + 'a>;

pub struct Column<'a, T> {
    name: &'static str,
    calc_width: Option<CalcWidth<'a, T>>,
    calc_value: CalcValue<'a, T>,
    max_width: Option<usize>,
    h_padding: Option<usize>,
    can_shrink: bool,
}

impl<'a, T> Column<'a, T> {
    pub fn new(name: &'static str, calc_value: CalcValue<'a, T>) -> Self {
        Self {
            name,
            calc_width: None,
            calc_value,
            max_width: None,
            h_padding: None,
            can_shrink: false,
        }
    }

    pub fn calc_width(self, calc_width: CalcWidth<'a, T>) -> Self {
        Self {
            calc_width: Some(calc_width),
            ..self
        }
    }

    // pub fn max_width(self, max_width: Option<usize>) -> Self {
    //     Self { max_width, ..self }
    // }

    pub fn h_padding(self, h_padding: Option<usize>) -> Self {
        Self { h_padding, ..self }
    }

    pub fn can_shrink(self, can_shrink: bool) -> Self {
        Self { can_shrink, ..self }
    }
}

impl<T> table::Column<T> for Column<'_, T> {
    fn name(&self) -> &str {
        self.name
    }

    fn calc_width(&self, value: &T) -> usize {
        if let Some(calc_width) = &self.calc_width {
            (calc_width)(value)
        } else {
            (self.calc_value)(value).len()
        }
    }

    fn calc_value<'a>(&self, value: &'a T) -> Cow<'a, str> {
        (self.calc_value)(value)
    }

    fn max_width(&self) -> Option<usize> {
        self.max_width
    }

    fn h_padding(&self) -> Option<usize> {
        self.h_padding
    }

    fn can_shrink(&self) -> bool {
        self.can_shrink
    }
}

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Field {
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

pub type PidAndInfo = (Pid, ProcessInfo);

impl Field {
    pub fn compare(self, (a_pid, a_info): &PidAndInfo, (b_pid, b_info): &PidAndInfo) -> Ordering {
        match self {
            Field::Pid => a_pid.cmp(b_pid),
            Field::ParentPid => a_info.parent_pid.cmp(&b_info.parent_pid),
            Field::Uid => a_info.uid.cmp(&b_info.uid),
            Field::Username => a_info.username.cmp(&b_info.username),
            Field::Path => a_info.path.cmp(&b_info.path),
            Field::CmdLine => a_info.cmd_line.cmp(&b_info.cmd_line),
            Field::Name => a_info.name.cmp(&b_info.name),
            Field::AnyName => a_info
                .cmd_line
                .cmp(&b_info.cmd_line)
                .then_with(|| a_info.name.cmp(&b_info.name))
                .then_with(|| a_info.path.cmp(&b_info.path))
                .then_with(|| (!a_info.is_defunct).cmp(&(!b_info.is_defunct))),
            Field::CpuUsage => a_info
                .cpu_usage
                .partial_cmp(&b_info.cpu_usage)
                .unwrap_or(Ordering::Equal),
            Field::MemUsage => a_info
                .mem_usage
                .partial_cmp(&b_info.mem_usage)
                .unwrap_or(Ordering::Equal),
            Field::CpuTime => a_info.cpu_time.cmp(&b_info.cpu_time),
            Field::VirtualMemSize => a_info.virtual_mem_size.cmp(&b_info.virtual_mem_size),
            Field::PhysicalMemSize => a_info.physical_mem_size.cmp(&b_info.physical_mem_size),
            Field::Tty => a_info.controlling_tty.cmp(&b_info.controlling_tty),
            Field::StartTime => a_info.start_time.cmp(&b_info.start_time),
        }
    }

    pub fn to_column(self, ps_compat: bool) -> Column<'static, PidAndInfo> {
        match self {
            Field::Pid => {
                Column::<PidAndInfo>::new("PID", Box::new(move |(pid, _)| pid.to_string().into()))
                    .calc_width(Box::new(move |(pid, _)| {
                        pid.raw().max(1).ilog10() as usize + 1
                    }))
                    .h_padding(Some(1))
            }

            Field::ParentPid => Column::<PidAndInfo>::new(
                if ps_compat { "PPID" } else { "Parent" },
                Box::new(move |(_, info)| match info.parent_pid.to_option() {
                    None => "-".into(),
                    Some(parent_pid) => parent_pid.to_string().into(),
                }),
            )
            .calc_width(Box::new(move |(_, info)| {
                match info.parent_pid.to_option() {
                    None => 1,
                    Some(parent_pid) => parent_pid.raw().max(1).ilog10() as usize + 1,
                }
            }))
            .h_padding(Some(1)),

            Field::Uid => Column::<PidAndInfo>::new(
                "UID",
                Box::new(move |(_, info)| match info.uid.to_option() {
                    None => "-".into(),
                    Some(uid) => uid.to_string().into(),
                }),
            )
            .calc_width(Box::new(move |(_, info)| match info.uid.to_option() {
                None => 1,
                Some(uid) => uid.raw().max(1).ilog10() as usize + 1,
            }))
            .h_padding(Some(1)),

            Field::Username => Column::<PidAndInfo>::new(
                if ps_compat { "USER" } else { "User" },
                Box::new(move |(_, info)| {
                    match info.username.to_option() {
                        None => "-",
                        Some(username) => username,
                    }
                    .into()
                }),
            )
            .h_padding(Some(1)),

            Field::Path => Column::<PidAndInfo>::new(
                if ps_compat { "PATH" } else { "Path" },
                Box::new(move |(_, info)| info.path.to_str().into()),
            )
            .can_shrink(true),

            Field::CmdLine => Column::<PidAndInfo>::new(
                if ps_compat { "COMMAND" } else { "Command line" },
                Box::new(move |(_, info)| info.cmd_line.to_str().into()),
            )
            .can_shrink(true),

            Field::Name => Column::<PidAndInfo>::new(
                if ps_compat { "NAME" } else { "Name" },
                Box::new(move |(_, info)| info.name.to_str().into()),
            )
            .can_shrink(true),

            Field::AnyName => Column::<PidAndInfo>::new(
                if ps_compat {
                    "CMD/[NAME]/<PATH>"
                } else {
                    "Cmd/[Name]/<Path>"
                },
                Box::new(move |(_, info)| {
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
            .can_shrink(true),

            Field::CpuUsage => Column::<PidAndInfo>::new(
                if ps_compat { "%CPU" } else { "CPU" },
                Box::new(move |(_, info)| match info.cpu_usage.to_option() {
                    None => "-".into(),
                    Some(cpu_usage) => if ps_compat {
                        format!(
                            "{:.precision$}",
                            cpu_usage * 100.0,
                            precision = if ps_compat { 1 } else { 2 }
                        )
                    } else {
                        format!(
                            "{:.precision$}%",
                            cpu_usage * 100.0,
                            precision = if ps_compat { 1 } else { 2 }
                        )
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1)),

            Field::CpuTime => Column::<PidAndInfo>::new(
                if ps_compat { "TIME" } else { "CPU time" },
                Box::new(move |(_, info)| match info.cpu_time.to_option() {
                    None => "-".into(),
                    Some(cpu_time) => {
                        let secs = cpu_time.as_secs_f64();
                        format!("{:02.0}:{:05.2}", (secs / 60.0).floor(), secs % 60.0)
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1)),

            Field::MemUsage => Column::<PidAndInfo>::new(
                if ps_compat { "%MEM" } else { "Mem" },
                Box::new(move |(_, info)| match info.mem_usage.to_option() {
                    None => "-".into(),
                    Some(mem_usage) => if ps_compat {
                        format!(
                            "{:.precision$}",
                            mem_usage * 100.0,
                            precision = if ps_compat { 1 } else { 2 }
                        )
                    } else {
                        format!(
                            "{:.precision$}%",
                            mem_usage * 100.0,
                            precision = if ps_compat { 1 } else { 2 }
                        )
                    }
                    .into(),
                }),
            )
            .h_padding(Some(1)),

            Field::VirtualMemSize => Column::<PidAndInfo>::new(
                if ps_compat { "VSZ" } else { "Virt mem" },
                Box::new(move |(_, info)| match info.virtual_mem_size.to_option() {
                    None => "-".into(),
                    Some(vm_size) => if ps_compat {
                        (*vm_size >> 10).to_string()
                    } else {
                        format_mem(*vm_size)
                    }
                    .into(),
                }),
            )
            .calc_width(Box::new(move |(_, info)| {
                match info.virtual_mem_size.to_option() {
                    None => 1,
                    Some(vm_size) => {
                        if ps_compat {
                            (*vm_size >> 10).max(1).ilog10() as usize + 1
                        } else {
                            format_mem(*vm_size).len()
                        }
                    }
                }
            }))
            .h_padding(Some(1)),

            Field::PhysicalMemSize => Column::<PidAndInfo>::new(
                if ps_compat { "RSS" } else { "Phys mem" },
                Box::new(move |(_, info)| match info.physical_mem_size.to_option() {
                    None => "-".into(),
                    Some(phys_size) => if ps_compat {
                        (*phys_size >> 10).to_string()
                    } else {
                        format_mem(*phys_size)
                    }
                    .into(),
                }),
            )
            .calc_width(Box::new(move |(_, info)| {
                match info.physical_mem_size.to_option() {
                    None => 1,
                    Some(phys_size) => {
                        if ps_compat {
                            (*phys_size >> 10).max(1).ilog10() as usize + 1
                        } else {
                            format_mem(*phys_size).len()
                        }
                    }
                }
            }))
            .h_padding(Some(1)),

            Field::Tty => Column::<PidAndInfo>::new(
                if ps_compat { "TT" } else { "TTY" },
                Box::new(move |(_, info)| match info.controlling_tty.to_option() {
                    None => "-".into(),
                    Some(None) => if ps_compat { "??" } else { "?" }.into(),
                    Some(Some(controlling_tty)) => controlling_tty.into(),
                }),
            )
            .h_padding(Some(1)),

            Field::StartTime => Column::<PidAndInfo>::new(
                if ps_compat { "STARTED" } else { "Start" },
                Box::new(move |(_, info)| match info.start_time.to_option() {
                    None => "-".into(),
                    Some(start_time) => {
                        let elapsed = start_time.elapsed().unwrap_or(Duration::ZERO);
                        let use_am_pm = true; // TODO
                        let format = if ps_compat {
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
            .h_padding(Some(1)),
        }
    }
}

#[derive(clap::Parser)]
pub struct TableArgs {
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
    pub cols: Vec<Field>,
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
    pub sort: Vec<Field>,
    #[arg(
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
    pub plain: bool,
    #[arg(
        action = ArgAction::Set,
        long = "ps",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to produce ps-compatible output for data.
    pub ps_compat: bool,
}

impl TableArgs {
    pub fn table_template(
        &self,
        options: &GlobalOptions,
    ) -> table::TableTemplate<PidAndInfo, Column<'static, PidAndInfo>> {
        let columns = self
            .cols
            .iter()
            .map(|column| column.to_column(self.ps_compat))
            .collect::<Vec<_>>();

        table::Builder::new()
            .style(if self.plain {
                table::Style::None
            } else if options.use_box_drawing {
                table::Style::BoxDrawing
            } else {
                table::Style::Ascii
            })
            .h_padding(2)
            .build(columns)
    }

    pub fn sorted_processes_info(&self, options: &GlobalOptions) -> Vec<PidAndInfo> {
        let mut processes_info =
            ProcessInfo::par_apply_filter(ProcessInfo::list_all(), &options.filter)
                .collect::<Vec<_>>();
        if !self.sort.is_empty() {
            processes_info.sort_by(|a, b| {
                self.sort
                    .iter()
                    .find_map(|column| Some(column.compare(a, b)).filter(|c| !c.is_eq()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        processes_info
    }
}
