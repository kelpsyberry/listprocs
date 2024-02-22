use super::GlobalOptions;
use crate::{utils::table, Pid, ProcessInfo};
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
                        ColumnName::ParentPid => a_info.parent_pid.cmp(&b_info.parent_pid),
                        ColumnName::Uid => a_info.uid.cmp(&b_info.uid),
                        ColumnName::Username => a_info.username.cmp(&b_info.username),
                        ColumnName::Path => a_info.path.cmp(&b_info.path),
                        ColumnName::CmdLine => a_info.cmd_line.cmp(&b_info.cmd_line),
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
                "Parent",
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
                "User",
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
                "Path",
                Box::new(|(_, info)| info.path.to_str().into()),
            )
            .can_shrink(true)
            .build(),
            ColumnName::CmdLine => table::ColumnBuilder::<(Pid, ProcessInfo)>::new(
                "Command line",
                Box::new(|(_, info)| info.cmd_line.to_str().into()),
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
