use super::{GlobalOptions, ProcessInfo};
use crate::{utils::truncate_string, CmdLine, Pid};
use std::{
    collections::{BTreeMap, HashMap},
    iter,
};

#[derive(clap::Parser)]
pub struct TreeArgs {
    #[arg(
        action = clap::ArgAction::Set,
        long = "ancestors",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to show all ancestors of visible processes, even if otherwise filtered out.
    include_ancestors: bool,
}

pub fn tree(options: GlobalOptions, args: TreeArgs) {
    #[derive(Debug)]
    struct Node(BTreeMap<Pid, Node>);

    fn create_tree<'a>(
        processes_info: impl IntoIterator<Item = (&'a Pid, &'a ProcessInfo)>,
        full_processes_info: &HashMap<Pid, ProcessInfo>,
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
        processes_info: &HashMap<Pid, ProcessInfo>,
        options: &GlobalOptions,
    ) {
        for (i, (pid, child_children)) in children.iter().enumerate() {
            let info = &processes_info[pid];
            let mut name = match info {
                ProcessInfo::Defunct => "<defunct>",
                ProcessInfo::Running(info) => match &info.cmd_line {
                    CmdLine::None | CmdLine::Unauthorized => &info.path,
                    CmdLine::Some(cmd_line) => cmd_line,
                },
            }
            .to_string();

            if let Some(max_len) = options.terminal_width.map(|width| {
                width
                    - (borders.chars().count()
                        + 2
                        + 1
                        + (pid.raw().max(1).ilog10() as usize + 1)
                        + 1)
            }) {
                truncate_string(&mut name, max_len);
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

    let processes_info_iter = ProcessInfo::list_all();
    let (root, processes_info) = if args.include_ancestors {
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
