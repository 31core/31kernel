/*!
 * Definition of syscall numbers and generic implementations.
*/

use crate::{task::Task, vfs::ROOT_VFS};

pub const SYSCALL_EXIT: u64 = 0;
pub const SYSCALL_OPEN: u64 = 1;
pub const SYSCALL_READ: u64 = 2;
pub const SYSCALL_WRITE: u64 = 3;
pub const SYSCALL_LSEEK: u64 = 4;
pub const SYSCALL_CLOSE: u64 = 5;
pub const SYSCALL_SLEEP: u64 = 6;
pub const SYSCALL_FORK: u64 = 7;
pub const SYSCALL_UNAME: u64 = 8;

pub const SYSCALL_RET_OK: isize = 0;
pub const SYSCALL_RET_ERR: isize = -1;

/**
 * Dispatches these syscalls, with the current [Task] struct:
 * * SYSCALL_OPEN
 * * SYSCALL_READ
 * * SYSCALL_WRITE
 * * SYSCALL_LSEEK
 * * SYSCALL_CLOSE
 * * SYSCALL_SLEEP
 * * SYSCALL_UNAME
 */
pub fn dispatch_with_task(
    current_task: &mut Task,
    syscall_num: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    _a3: u64,
) -> Option<u64> {
    match syscall_num {
        SYSCALL_OPEN => {
            let path = current_task.copy_user_string(a0 as usize);
            Some(syscall_open(current_task, &path) as u64)
        }
        SYSCALL_READ => {
            let mut buf = alloc::vec![0; a2 as usize];
            let ret = syscall_read(current_task, a0, &mut buf) as u64;
            current_task.copy_to_user(a1 as usize, &buf);
            Some(ret)
        }
        SYSCALL_WRITE => {
            let mut buf = alloc::vec![0; a2 as usize];
            current_task.copy_from_user(a1 as usize, &mut buf);
            Some(syscall_write(current_task, a0, &buf) as u64)
        }
        SYSCALL_LSEEK => Some(syscall_lseek(current_task, a0, a1) as u64),
        SYSCALL_CLOSE => {
            let mut buf = alloc::vec![0; a2 as usize];
            current_task.copy_from_user(a1 as usize, &mut buf);
            Some(syscall_close(current_task, a0) as u64)
        }
        SYSCALL_SLEEP => {
            syscall_sleep(current_task, a0);
            None
        }
        SYSCALL_UNAME => {
            syscall_uname(current_task, a0);
            None
        }
        _ => None,
    }
}

pub fn syscall_open(current_task: &mut Task, path: &str) -> isize {
    let mut vfs_guard = ROOT_VFS.lock();
    let vfs = unsafe { vfs_guard.assume_init_mut() };
    if let Ok(fd) = vfs.open(path) {
        current_task.fds.add(fd) as isize
    } else {
        SYSCALL_RET_ERR
    }
}

pub fn syscall_read(current_task: &mut Task, fd: u64, buf: &mut [u8]) -> isize {
    let mut vfs_guard = ROOT_VFS.lock();
    let vfs = unsafe { vfs_guard.assume_init_mut() };
    if let Some(fd) = current_task.fds.get_mut(fd as usize)
        && let Ok(size) = vfs.read(fd, buf)
    {
        size as isize
    } else {
        SYSCALL_RET_ERR
    }
}

pub fn syscall_write(current_task: &mut Task, fd: u64, buf: &[u8]) -> isize {
    let mut vfs_guard = ROOT_VFS.lock();
    let vfs = unsafe { vfs_guard.assume_init_mut() };
    if let Some(fd) = current_task.fds.get_mut(fd as usize)
        && let Ok(size) = vfs.write(fd, buf)
    {
        size as isize
    } else {
        SYSCALL_RET_ERR
    }
}

pub fn syscall_lseek(current_task: &mut Task, fd: u64, position: u64) -> isize {
    if let Some(fd) = current_task.fds.get_mut(fd as usize) {
        fd.offset = position;
        SYSCALL_RET_OK
    } else {
        SYSCALL_RET_ERR
    }
}

pub fn syscall_close(current_task: &mut Task, fd: u64) -> isize {
    let mut vfs_guard = ROOT_VFS.lock();
    let vfs = unsafe { vfs_guard.assume_init_mut() };
    if let Some(fd) = current_task.fds.get(fd as usize)
        && let Ok(_) = vfs.close(fd)
    {
        SYSCALL_RET_OK
    } else {
        SYSCALL_RET_ERR
    }
}

pub fn syscall_sleep(current_task: &mut Task, timestamp: u64) {
    let next_time = crate::time::get_sys_time() + timestamp;
    current_task.next_schedule = Some(next_time);
}

const UTS_STRING_LEN: usize = 65;

#[repr(C)]
struct Utsname {
    sysname: [u8; UTS_STRING_LEN],
    nodename: [u8; UTS_STRING_LEN],
    release: [u8; UTS_STRING_LEN],
    version: [u8; UTS_STRING_LEN],
    machine: [u8; UTS_STRING_LEN],
}

impl Default for Utsname {
    fn default() -> Self {
        Utsname {
            sysname: [0; UTS_STRING_LEN],
            nodename: [0; UTS_STRING_LEN],
            release: [0; UTS_STRING_LEN],
            version: [0; UTS_STRING_LEN],
            machine: [0; UTS_STRING_LEN],
        }
    }
}

const UNAME_SYSNAME: &[u8] = b"31kernel";
const UNAME_RELEASE: &[u8] = b"0.1.0";

#[cfg(debug_assertions)]
const UNAME_VERSION: &[u8] = b"31kernel version 0.1.0 (Debug channel)";
#[cfg(not(debug_assertions))]
const UNAME_VERSION: &[u8] = b"31kernel version 0.1.0 (Release channel)";

#[cfg(target_arch = "aarch64")]
const UNAME_MACHINE: &[u8] = b"arm64";
#[cfg(target_arch = "riscv64")]
const UNAME_MACHINE: &[u8] = b"riscv64";
#[cfg(target_arch = "x86_64")]
const UNAME_MACHINE: &[u8] = b"x86_64";

pub fn syscall_uname(current_task: &mut Task, uts_ptr: u64) {
    let mut uts = Utsname::default();
    uts.sysname[..UNAME_SYSNAME.len()].copy_from_slice(UNAME_SYSNAME);
    uts.release[..UNAME_RELEASE.len()].copy_from_slice(UNAME_RELEASE);
    uts.version[..UNAME_VERSION.len()].copy_from_slice(UNAME_VERSION);
    uts.machine[..UNAME_MACHINE.len()].copy_from_slice(UNAME_MACHINE);

    let uts_bytes = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(uts) as *const u8,
            core::mem::size_of_val(&uts),
        )
    };
    current_task.copy_to_user(uts_ptr as usize, uts_bytes);
}
