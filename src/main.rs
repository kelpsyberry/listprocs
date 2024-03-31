#![warn(clippy::all)]

mod cli;
mod ffi;
use ffi::{Pid, Uid};
mod info;
mod utils;
use info::*;
mod process_info;
use process_info::*;

fn main() {
    cli::main();
}
