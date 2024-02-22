mod utils;

#[cfg(target_vendor = "apple")]
mod darwin;
#[cfg(unix)]
mod unix;
#[cfg(target_vendor = "apple")]
pub use darwin::*;
