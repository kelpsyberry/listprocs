pub use super::unix::*;

use crate::{Info, ProcessInfo, RunningProcessInfo};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read},
    os::unix::ffi::OsStrExt,
    os::unix::fs::MetadataExt,
    str,
};

impl Pid {
    pub fn all_active() -> Result<impl Iterator<Item = Self>, io::Error> {
        Ok(fs::read_dir("/proc")?
            .filter_map(|entry| Some(Pid(entry.ok()?.file_name().to_str()?.parse().ok()?))))
    }

    fn status(self) -> Result<(bool, Uid, Pid), io::Error> {
        let mut file = fs::File::open(format!("/proc/{self}/stat"))?;

        let metadata = file.metadata()?;
        let uid = Uid(metadata.uid() as _);

        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)?;

        let name_end = bytes.len()
            - 1
            - bytes
                .iter()
                .rev()
                .position(|b| *b == b')')
                .ok_or(io::ErrorKind::InvalidData)?;
        let fields = bytes[name_end + 2..]
            .split(|b| *b == b' ')
            .map(|b| str::from_utf8(b).expect("/proc/pid/stat contained invalid UTF-8"))
            .collect::<Vec<_>>();
        let is_defunct = fields[0] == "Z";
        let parent_pid = fields[1]
            .parse()
            .expect("couldn't parse /proc/pid/stat ppid");

        Ok((is_defunct, uid, parent_pid))
    }

    fn cmd_line(self) -> Result<Info<Option<Vec<OsString>>>, io::Error> {
        let bytes = fs::read(format!("/proc/{self}/cmdline"))?;
        Ok(Info::Some((!bytes.is_empty()).then(|| {
            bytes
                .split(|b| *b == 0)
                .map(|b| OsStr::from_bytes(b).to_os_string())
                .collect::<Vec<_>>()
        })))
    }

    fn path(self) -> Result<Info<OsString>, io::Error> {
        let result = match fs::read_link(format!("/proc/{self}/exe")) {
            Ok(path) => path,
            Err(err) => {
                if err.kind() == io::ErrorKind::PermissionDenied {
                    return Ok(Info::Unauthorized);
                } else {
                    return Err(err);
                }
            }
        };
        Ok(Info::Some(result.into_os_string()))
    }

    pub fn info(self) -> Result<ProcessInfo, io::Error> {
        let (is_defunct, uid, parent_pid) = dbg!(dbg!(self).status())?;
        if is_defunct {
            return Ok(ProcessInfo::Defunct);
        }

        let path = dbg!(self.path())?;
        let username = dbg!(uid.username())?;
        let cmd_line = dbg!(self.cmd_line())?;

        Ok(ProcessInfo::Running(RunningProcessInfo {
            parent_pid,
            uid,
            username: username.to_string_lossy().into_owned(),
            path: match path {
                Info::Some(path) => Info::Some(path.to_string_lossy().into_owned()),
                Info::Unauthorized => Info::Unauthorized,
            },
            cmd_line: match cmd_line {
                Info::Unauthorized => Info::Unauthorized,
                Info::Some(cmd_line) => {
                    Info::Some(cmd_line.map(|cmd_line| {
                        cmd_line.join(OsStr::new(" ")).to_string_lossy().to_string()
                    }))
                }
            },
        }))
    }
}