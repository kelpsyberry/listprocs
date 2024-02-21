#![warn(clippy::all)]

mod cli;
mod ffi;
mod utils;

fn main() {
    cli::main();
}
