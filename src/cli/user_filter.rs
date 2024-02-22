use crate::Uid;
use clap::{
    builder::TypedValueParser,
    error::{ContextKind, ContextValue, ErrorKind},
    Arg, Command, Error,
};

#[derive(Clone)]
pub enum UserFilter {
    Uid(Uid),
    Username(String),
}

#[derive(Clone, Copy)]
pub struct Parser;

fn invalid_value(cmd: &Command, bad_val: String, arg: String) -> Error {
    let mut err = Error::new(ErrorKind::InvalidValue).with_cmd(cmd);
    err.insert(ContextKind::InvalidArg, ContextValue::String(arg));
    err.insert(ContextKind::InvalidValue, ContextValue::String(bad_val));
    err
}

impl TypedValueParser for Parser {
    type Value = UserFilter;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, Error> {
        let value = value.to_str().ok_or_else(|| {
            invalid_value(
                cmd,
                value.to_string_lossy().into_owned(),
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            )
        })?;
        if value.is_empty() {
            Err(invalid_value(
                cmd,
                value.to_string(),
                arg.map(ToString::to_string)
                    .unwrap_or_else(|| "...".to_owned()),
            ))
        } else if value == "-" {
            Ok(UserFilter::Uid(Uid::current()))
        } else if let Ok(uid) = value.parse::<Uid>() {
            Ok(UserFilter::Uid(uid))
        } else {
            Ok(UserFilter::Username(value.to_string()))
        }
    }
}
