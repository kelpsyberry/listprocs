use std::{ffi::c_int, io};

fn check_valid<T>(result: T, is_valid: bool) -> Result<T, io::Error> {
    if is_valid {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn check_pos_zero(result: c_int) -> Result<c_int, io::Error> {
    check_valid(result, result >= 0)
}

pub fn check_pos(result: c_int) -> Result<c_int, io::Error> {
    check_valid(result, result > 0)
}

pub fn check_nonnull<T>(result: *mut T) -> Result<*mut T, io::Error> {
    check_valid(result, !result.is_null())
}
