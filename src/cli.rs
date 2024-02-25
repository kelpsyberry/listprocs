mod list;
use list::ListArgs;
mod tree;
use tree::TreeArgs;
mod user_filter;
use user_filter::UserFilter;

use crate::{Pid, ProcessInfo, Uid};
use clap::{
    builder::{StringValueParser, TypedValueParser},
    ArgAction, Parser,
};
use rayon::prelude::*;
use regex::{Regex, RegexBuilder};
use std::borrow::Borrow;

struct ProcessFilter {
    regex: Option<Regex>,
    invert_regex: bool,
    uids: Vec<Uid>,
    usernames: Vec<String>,
    include_defunct: bool,
    #[cfg(target_vendor = "apple")]
    include_sip: bool,
}

impl ProcessInfo {
    fn filter(&self, _pid: Pid, filter: &ProcessFilter) -> bool {
        (filter.include_defunct || (!self.is_defunct))
            && ({
                #[cfg(target_vendor = "apple")]
                {
                    filter.include_sip || !self.is_sip_protected()
                }
                #[cfg(not(target_vendor = "apple"))]
                true
            })
            && {
                filter.usernames.is_empty()
                    || self
                        .username
                        .to_option()
                        .map_or(false, |username| filter.usernames.contains(username))
            }
            && {
                filter.uids.is_empty()
                    || self
                        .uid
                        .to_option()
                        .map_or(false, |uid| filter.uids.contains(uid))
            }
            && {
                filter.regex.as_ref().map_or(true, |regex| {
                    filter.invert_regex != regex.is_match(self.path.to_str())
                        || regex.is_match(self.cmd_line.to_str())
                })
            }
    }

    fn apply_filter<'a, P: Borrow<Pid>, I: Borrow<ProcessInfo>>(
        info: impl Iterator<Item = (P, I)> + 'a,
        filter: &'a ProcessFilter,
    ) -> impl Iterator<Item = (P, I)> + 'a {
        info.filter(|(pid, info)| info.borrow().filter(*pid.borrow(), filter))
    }

    fn par_apply_filter<'a, P: Borrow<Pid> + Send, I: Borrow<ProcessInfo> + Send>(
        info: impl ParallelIterator<Item = (P, I)> + 'a,
        filter: &'a ProcessFilter,
    ) -> impl ParallelIterator<Item = (P, I)> + 'a {
        info.filter(|(pid, info)| info.borrow().filter(*pid.borrow(), filter))
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

#[cfg(target_vendor = "apple")]
fn include_sip_long_help() -> String {
    format!(
        "Whether to include SIP-protected executables.

Executables are considered SIP-protected if they're in any of the following paths: {}.
Defaults to true if using a regex, and false otherwise.",
        ProcessInfo::SIP_PREFIXES.join(", ")
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
    /// If present, only show processes belonging to the specified UIDs or usernames.
    ///
    /// A hyphen or no value will select the current UID); if unspecified, processes won't be
    /// filtered by user.
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
    #[cfg(target_vendor = "apple")]
    #[arg(
        global = true,
        action = ArgAction::Set,
        long = "sip",
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
        default_value_if("regex", clap::builder::ArgPredicate::IsPresent, Some("true")),
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
    #[arg(
        global = true,
        action = ArgAction::Set,
        long,
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to always use unlimited width for output, even when it's to an interactive terminal.
    wide: bool,

    #[command(subcommand)]
    subcommand: Option<Subcommand>,
    #[command(flatten)]
    list_args: ListArgs,
}

pub fn main() {
    let args = Args::parse();

    let mut uids = Vec::new();
    let mut usernames = Vec::new();
    for filter in args.user_filter.into_iter().flatten() {
        match filter {
            UserFilter::Uid(uid) => uids.push(uid),
            UserFilter::Username(username) => usernames.push(username),
        }
    }

    let options = GlobalOptions {
        filter: ProcessFilter {
            regex: args.regex,
            invert_regex: args.invert_matches,
            uids,
            usernames,
            include_defunct: args.include_defunct,
            #[cfg(target_vendor = "apple")]
            include_sip: args.include_sip,
        },
        use_box_drawing: !args.use_ascii,
        terminal_width: if args.wide {
            None
        } else {
            terminal_size::terminal_size().map(|size| size.0 .0 as usize)
        },
    };

    match args.subcommand {
        Some(Subcommand::Tree(tree_args)) => tree::tree(options, tree_args),
        None => list::list(options, args.list_args),
    }
}
