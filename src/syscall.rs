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

pub unsafe fn syscall_lseek<P>(current_task: &mut Task<P>, fd: u64, position: u64) -> isize
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

pub unsafe fn syscall_sleep<P>(current_task: &mut Task<P>, timestamp: u64)
where
    P: Paging + Send,
{
    let next_time = crate::time::get_sys_time() + timestamp;
    current_task.next_schedule = Some(next_time);
}
