pub mod table;

use std::iter;

pub fn mark_first<T>(iter: impl IntoIterator<Item = T>) -> impl Iterator<Item = (bool, T)> {
    iter::once(true).chain(iter::repeat(false)).zip(iter)
}
