use super::{common::TableArgs, GlobalOptions};
use clap::builder::ArgAction;

#[derive(clap::Parser)]
pub struct ListArgs {
    #[arg(
        action = ArgAction::Set,
        long,
        value_name = "BOOL",
        require_equals = true,
        num_args = 0..2,
        default_missing_value = "true",
        default_value = "false",
    )]
    /// Whether to kill a random process.
    kill_random: bool,

    #[command(flatten)]
    table_args: TableArgs,
}

pub fn list(options: GlobalOptions, args: ListArgs) {
    let processes_info = args.table_args.sorted_processes_info(&options);

    if args.kill_random && !processes_info.is_empty() {
        let index = unsafe {
            libc::srand(libc::time(std::ptr::null_mut()) as libc::c_uint);
            libc::rand()
        } as usize
            % processes_info.len();
        let (pid, info) = &processes_info[index];
        println!(
            "Killing PID {pid} ({}) (you literally had to explicitly ask for it)",
            info.cmd_line.to_str()
        );
        if let Ok(mut child) = std::process::Command::new("sudo")
            .arg("kill")
            .arg("-9")
            .arg(pid.raw().to_string())
            .spawn()
        {
            if child.wait().is_err() {
                println!(":(");
            }
        }
    }

    print!(
        "{}",
        args.table_args
            .table_template(&options)
            .format(&processes_info, options.terminal_width())
    );
}
