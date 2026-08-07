/*!
 * Definition of syscall numbers and generic implementations.
*/

use crate::{page::Paging, task::Task, vfs::ROOT_VFS};

pub const SYSCALL_EXIT: u64 = 0;
pub const SYSCALL_OPEN: u64 = 1;
pub const SYSCALL_READ: u64 = 2;
pub const SYSCALL_WRITE: u64 = 3;
pub const SYSCALL_LSEEK: u64 = 4;
pub const SYSCALL_CLOSE: u64 = 5;
pub const SYSCALL_SLEEP: u64 = 6;
pub const SYSCALL_FORK: u64 = 7;

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
 */
pub fn dispatch_with_task<P>(
    current_task: &mut Task<P>,
    syscall_num: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    _a3: u64,
) -> Option<u64>
where
    P: Paging + Send,
{
    match syscall_num {
        SYSCALL_OPEN => unsafe {
            let path = current_task.copy_user_string(a0 as usize);
            Some(syscall_open(current_task, &path) as u64)
        },
        SYSCALL_READ => unsafe {
            let mut buf = alloc::vec![0; a2 as usize];
            let ret = syscall_read(current_task, a0, &mut buf) as u64;
            current_task.copy_to_user(a1 as usize, &buf);
            Some(ret)
        },
        SYSCALL_WRITE => unsafe {
            let mut buf = alloc::vec![0; a2 as usize];
            current_task.copy_from_user(a1 as usize, &mut buf);
            Some(syscall_write(current_task, a0, &buf) as u64)
        },
        SYSCALL_LSEEK => Some(syscall_lseek(current_task, a0, a1) as u64),
        SYSCALL_CLOSE => unsafe {
            let mut buf = alloc::vec![0; a2 as usize];
            current_task.copy_from_user(a1 as usize, &mut buf);
            Some(syscall_close(current_task, a0) as u64)
        },
        SYSCALL_SLEEP => {
            syscall_sleep(current_task, a0);
            None
        }
        _ => None,
    }
}

pub unsafe fn syscall_open<P>(current_task: &mut Task<P>, path: &str) -> isize
where
    P: Paging + Send,
{
    let mut vfs_guard = ROOT_VFS.lock();
    let vfs = unsafe { vfs_guard.assume_init_mut() };
    if let Ok(fd) = vfs.open(path) {
        current_task.fds.add(fd) as isize
    } else {
        SYSCALL_RET_ERR
    }
}

pub unsafe fn syscall_read<P>(current_task: &mut Task<P>, fd: u64, buf: &mut [u8]) -> isize
where
    P: Paging + Send,
{
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

pub unsafe fn syscall_write<P>(current_task: &mut Task<P>, fd: u64, buf: &[u8]) -> isize
where
    P: Paging + Send,
{
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

pub fn syscall_lseek<P>(current_task: &mut Task<P>, fd: u64, position: u64) -> isize
where
    P: Paging + Send,
{
    if let Some(fd) = current_task.fds.get_mut(fd as usize) {
        fd.offset = position;
        SYSCALL_RET_OK
    } else {
        SYSCALL_RET_ERR
    }
}

pub unsafe fn syscall_close<P>(current_task: &mut Task<P>, fd: u64) -> isize
where
    P: Paging + Send,
{
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

pub fn syscall_sleep<P>(current_task: &mut Task<P>, timestamp: u64)
where
    P: Paging + Send,
{
    let next_time = crate::time::get_sys_time() + timestamp;
    current_task.next_schedule = Some(next_time);
}
