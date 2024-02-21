use super::{GlobalOptions, ProcessInfo};
use crate::ffi::{self, Pid};
use crate::utils::table;
use std::cmp::Ordering;

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
}

pub fn list(options: GlobalOptions, args: ListArgs) {
    fn ord_ne(ordering: Ordering) -> Option<Ordering> {
        match ordering {
            Ordering::Equal => None,
            _ => Some(ordering),
        }
    }

    let mut processes_info =
        ProcessInfo::apply_filter(ProcessInfo::list_all(), &options.filter).collect::<Vec<_>>();
    if !args.sort.is_empty() {
        processes_info.sort_by(|(a_pid, a_info), (b_pid, b_info)| {
            args.sort
                .iter()
                .find_map(|column| {
                    ord_ne(match column {
                        ColumnName::Pid => a_pid.cmp(b_pid),
                        ColumnName::ParentPid => a_info.cmp_by(b_info, |a_info, b_info| {
                            a_info.parent_pid.cmp(&b_info.parent_pid)
                        }),
                        ColumnName::Uid => {
                            a_info.cmp_by(b_info, |a_info, b_info| a_info.uid.cmp(&b_info.uid))
                        }
                        ColumnName::Username => a_info.cmp_by(b_info, |a_info, b_info| {
                            a_info.username.cmp(&b_info.username)
                        }),
                        ColumnName::Path => {
                            a_info.cmp_by(b_info, |a_info, b_info| a_info.path.cmp(&b_info.path))
                        }
                        ColumnName::CmdLine => a_info.cmp_by(b_info, |a_info, b_info| {
                            let kind = |cmd_line: &ffi::CmdLine<String>| match cmd_line {
                                ffi::CmdLine::None => 0,
                                ffi::CmdLine::Unauthorized => 1,
                                ffi::CmdLine::Some(_) => 2,
                            };
                            ord_ne(kind(&a_info.cmd_line).cmp(&kind(&b_info.cmd_line)))
                                .unwrap_or_else(|| a_info.cmd_line_str().cmp(b_info.cmd_line_str()))
                        }),
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
            .calc_width(Box::new(|(pid, _)| (*pid).max(1).ilog10() as usize + 1))
            .h_padding(Some(1))
            .build(),
            ColumnName::ParentPid => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "Parent",
                Box::new(|(_, info)| match info {
                    ProcessInfo::Defunct => "-".into(),
                    ProcessInfo::Running(info) => info.parent_pid.to_string().into(),
                }),
            )
            .calc_width(Box::new(|(pid, _)| (*pid).max(1).ilog10() as usize + 1))
            .h_padding(Some(1))
            .build(),
            ColumnName::Uid => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "UID",
                Box::new(|(_, info)| match info {
                    ProcessInfo::Defunct => "-".into(),
                    ProcessInfo::Running(info) => info.uid.to_string().into(),
                }),
            )
            .calc_width(Box::new(|(_, info)| match info {
                ProcessInfo::Defunct => 1,
                ProcessInfo::Running(info) => info.uid.max(1).ilog10() as usize + 1,
            }))
            .h_padding(Some(1))
            .build(),
            ColumnName::Username => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
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
            ColumnName::Path => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
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
            ColumnName::CmdLine => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "Command line",
                Box::new(|(_, info)| info.cmd_line_str().into()),
            )
            .can_shrink(true)
            .build(),
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
