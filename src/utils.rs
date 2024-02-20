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
