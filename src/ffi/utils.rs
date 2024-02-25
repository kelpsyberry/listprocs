use std::{ffi::c_int, io};

fn check_valid<T>(result: T, is_valid: bool) -> io::Result<T> {
    if is_valid {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn check_pos_zero(result: c_int) -> io::Result<c_int> {
    check_valid(result, result >= 0)
}

pub fn check_pos(result: c_int) -> io::Result<c_int> {
    check_valid(result, result > 0)
}

pub fn check_nonnull<T>(result: *mut T) -> io::Result<*mut T> {
    check_valid(result, !result.is_null())
}

macro_rules! memo {
    ($t: ty, $value: expr) => {{
        static VALUE: ::std::sync::OnceLock<$t> = ::std::sync::OnceLock::new();
        if let ::std::option::Option::Some(result) = VALUE.get() {
            *result
        } else {
            let _ = VALUE.set($value);
            *VALUE.get().unwrap()
        }
    }};
}
