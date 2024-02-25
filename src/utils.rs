pub mod table;

use std::iter;

pub fn mark_first<T>(iter: impl IntoIterator<Item = T>) -> impl Iterator<Item = (bool, T)> {
    iter::once(true).chain(iter::repeat(false)).zip(iter)
}

pub fn truncate_string(string: &mut String, max_len: usize) {
    if string.chars().count() > max_len {
        string.truncate(max_len - 1);
        string.push('…');
    }
}

pub fn format_mem(mem: u64) -> String {
    let prefix = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let log1024 = (63 - mem.max(1).leading_zeros()) / 10;
    format!(
        "{:.01} {}",
        mem as f64 / 2.0_f64.powi((10 * log1024) as i32),
        prefix[log1024 as usize]
    )
}
