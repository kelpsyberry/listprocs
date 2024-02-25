pub use super::unix::*;

use super::utils::check_pos_zero;
use crate::{Info, ProcessInfo};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Read},
    mem::MaybeUninit,
    num::ParseIntError,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    str,
    time::{Duration, SystemTime},
};

struct Status {
    uid: Uid,
    name: OsString,
    state: u8,
    parent_pid: Pid,
    tty_dev_number: i32,
    cpu_user_time: u64,
    cpu_system_time: u64,
    start_time: u64,
    vm_size: u64,
    rss: u64,
}

extern "C" {
    fn getpagesize() -> *mut libc::c_int;
}

fn uptime() -> io::Result<Duration> {
    let content = fs::read_to_string(format!("/proc/uptime"))?;
    let uptime_str = content.split_once(' ').ok_or(io::ErrorKind::InvalidData)?.0;
    Ok(Duration::from_secs_f64(
        uptime_str.parse().map_err(|_| io::ErrorKind::InvalidData)?,
    ))
}

fn seconds_to_ticks() -> u64 {
    memo!(u64, (unsafe { libc::sysconf(libc::_SC_CLK_TCK) }) as u64)
}

fn ticks_to_duration(ticks: u128, seconds_to_ticks: u64) -> Duration {
    let nanos = ticks * 1_000_000_000 / seconds_to_ticks as u128;
    Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn page_size() -> u64 {
    memo!(u64, (unsafe { getpagesize() }) as u64)
}

fn total_ram() -> io::Result<u64> {
    Ok(memo!(u64, unsafe {
        let mut result = MaybeUninit::<libc::sysinfo>::uninit();
        check_pos_zero(libc::sysinfo(result.as_mut_ptr()))?;
        result.assume_init().totalram
    }))
}

fn device_name(dev_number: u32) -> String {
    let major = (dev_number >> 8) as u8;
    let minor = (dev_number & 0xFF) | (dev_number >> 20 << 8);
    match (major, minor) {
        (3, 0..=0xFF) => format!(
            "/dev/tty{}{}",
            b"pqrstuvwxyzabcde"[minor as usize >> 4] as char,
            b"0123456789abcdef"[minor as usize & 0xF] as char
        ),
        (4, 0..=63) => format!("/dev/tty{}", minor),
        (4, 64..=0xFF) => format!("/dev/ttyS{}", minor - 64),
        (136..=143, 0..=0xFF) => {
            format!("/dev/pts/{}", minor + (major as u32 - 136) * 0x100)
        }
        // TODO
        _ => format!("{}.{}", major, minor),
    }
}

impl Pid {
    pub fn all_active() -> io::Result<impl Iterator<Item = Self>> {
        Ok(fs::read_dir("/proc")?
            .filter_map(|entry| Some(Pid(entry.ok()?.file_name().to_str()?.parse().ok()?))))
    }

    fn status(self) -> io::Result<Status> {
        let mut file = fs::File::open(format!("/proc/{self}/stat"))?;

        let metadata = file.metadata()?;
        let uid = Uid(metadata.uid() as _);

        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)?;

        let name_start = bytes
            .iter()
            .position(|b| *b == b'(')
            .ok_or(io::ErrorKind::InvalidData)?
            + 1;
        let name_end = bytes.len()
            - 1
            - bytes
                .iter()
                .rev()
                .position(|b| *b == b')')
                .ok_or(io::ErrorKind::InvalidData)?;
        let name = OsStr::from_bytes(&bytes[name_start..name_end]).to_os_string();

        let fields = bytes[name_end + 2..]
            .split(|b| *b == b' ')
            .map(|b| str::from_utf8(b).expect("/proc/pid/stat contained invalid UTF-8"))
            .collect::<Vec<_>>();

        (|| -> Result<Status, ParseIntError> {
            Ok(Status {
                uid,
                name,
                state: fields[0].as_bytes()[0],
                parent_pid: fields[1].parse()?,
                tty_dev_number: fields[4].parse()?,
                cpu_user_time: fields[11].parse()?,
                cpu_system_time: fields[12].parse()?,
                start_time: fields[19].parse()?,
                vm_size: fields[20].parse()?,
                rss: fields[21].parse()?,
            })
        })()
        .map_err(|_| io::ErrorKind::InvalidData.into())
    }

    fn cmd_line(self) -> io::Result<Info<Option<Vec<OsString>>>> {
        let bytes = fs::read(format!("/proc/{self}/cmdline"))?;
        Ok(Info::Some((!bytes.is_empty()).then(|| {
            bytes
                .split(|b| *b == 0)
                .map(|b| OsStr::from_bytes(b).to_os_string())
                .collect::<Vec<_>>()
        })))
    }

    fn path(self) -> io::Result<Info<Option<OsString>>> {
        let result = match fs::read_link(format!("/proc/{self}/exe")) {
            Ok(path) => path,
            Err(err) => {
                return match err.kind() {
                    io::ErrorKind::PermissionDenied => Ok(Info::Unauthorized),
                    io::ErrorKind::NotFound => Ok(Info::Some(None)),
                    _ => Err(err),
                };
            }
        };
        Ok(Info::Some(Some(result.into_os_string())))
    }

    pub fn info(self) -> io::Result<ProcessInfo> {
        let status = self.status()?;
        let is_defunct = status.state == b'Z';
        let username = status.uid.username()?.to_string_lossy().into_owned();
        let name = status.name.to_string_lossy().into_owned();

        let uptime = uptime()?;
        let seconds_to_ticks = seconds_to_ticks();
        let system_startup_time = SystemTime::now() - uptime;
        let start_time =
            system_startup_time + ticks_to_duration(status.start_time as u128, seconds_to_ticks);
        let running_time = start_time.elapsed().ok();
        let cpu_time = ticks_to_duration(
            status.cpu_user_time as u128 + status.cpu_system_time as u128,
            seconds_to_ticks,
        );
        let cpu_usage = if let Some(elapsed) = running_time {
            cpu_time.as_secs_f64() / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let page_size = page_size();
        let physical_memory_max_size = total_ram()?;
        let virtual_mem_size = status.vm_size;
        let physical_mem_size = status.rss * page_size;
        let mem_usage = physical_mem_size as f64 / physical_memory_max_size as f64;

        let controlling_tty = if status.tty_dev_number == 0 {
            None
        } else {
            Some(device_name(status.tty_dev_number as u32))
        };

        if is_defunct {
            return Ok(ProcessInfo {
                is_defunct,
                parent_pid: Info::Some(status.parent_pid),
                uid: Info::Some(status.uid),
                username: Info::Some(username),
                path: Info::Defunct,
                cmd_line: Info::Defunct,
                name: Info::Some(name),
                cpu_usage: Info::Some(cpu_usage),
                cpu_time: Info::Some(cpu_time),
                mem_usage: Info::Some(mem_usage),
                virtual_mem_size: Info::Some(virtual_mem_size),
                physical_mem_size: Info::Some(physical_mem_size),
                controlling_tty: Info::Some(controlling_tty),
                start_time: Info::Some(start_time),
            });
        }

        let path = self.path()?;
        let cmd_line = self.cmd_line()?;
        let cmd_line_str = cmd_line.map(|cmd_line_opt| {
            cmd_line_opt.map(|cmd_line| {
                cmd_line
                    .join(OsStr::new(" "))
                    .to_string_lossy()
                    .into_owned()
            })
        });

        Ok(ProcessInfo {
            is_defunct,
            parent_pid: Info::Some(status.parent_pid),
            uid: Info::Some(status.uid),
            username: Info::Some(username),
            path: path.map(|path| path.map(|path| path.to_string_lossy().into_owned())),
            cmd_line: cmd_line_str,
            name: Info::Some(name),
            cpu_usage: Info::Some(cpu_usage),
            cpu_time: Info::Some(cpu_time),
            mem_usage: Info::Some(mem_usage),
            virtual_mem_size: Info::Some(virtual_mem_size),
            physical_mem_size: Info::Some(physical_mem_size),
            controlling_tty: Info::Some(controlling_tty),
            start_time: Info::Some(start_time),
        })
    }
}
