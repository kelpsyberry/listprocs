#[allow(dead_code)]
mod utils;

#[cfg(target_vendor = "apple")]
mod darwin;
#[cfg(unix)]
mod unix;
#[cfg(target_vendor = "apple")]
pub use darwin::*;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;
