use super::{common::TableArgs, GlobalOptions};
use std::{thread::sleep, time::Duration};

#[derive(clap::Parser)]
pub struct WatchArgs {
    #[arg(
        short,
        long,
        value_name = "SECONDS",
        require_equals = true,
        default_value = "1"
    )]
    interval_secs: f64,

    #[command(flatten)]
    table_args: TableArgs,
}

pub fn watch(options: GlobalOptions, args: WatchArgs) {
    let mut table_template = args.table_args.table_template(&options);

    let interval = Duration::from_secs_f64(args.interval_secs);

    loop {
        let processes_info = args.table_args.sorted_processes_info(&options);
        print!(
            "\x1b[2J\x1b[H{}",
            table_template.format(&processes_info, options.terminal_width())
        );
        sleep(interval);
    }
}
