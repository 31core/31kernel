/*!
 * devfs filesystem usually mounted on `/dev`
 */

use crate::{
    device::DEVICE_MGR,
    kmsg::KMSG,
    lock_uinit,
    path::Path,
    rand::{GLOBAL_RNG, RandomGenerator},
    vfs::{File, FileSystem, FileType, FsError},
};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use core::result::Result;

pub const CHAR_DEV_MAJOR: usize = 1;

#[derive(Default)]
pub struct DevFS {
    pub fds: BTreeMap<u64, String>,
    /** Name => (Major, Minor) */
    pub devs: BTreeMap<String, (usize, usize)>,
}

const DEVFS_FILES: [&str; 5] = ["zero", "null", "kmsg", "random", "urandom"];

impl FileSystem for DevFS {
    fn create(&mut self, _path: &Path) -> Result<File, FsError> {
        Err(FsError::NotSupported)
    }
    fn open(&mut self, path: &Path) -> Result<File, FsError> {
        let dev = path.iter().collect::<Vec<&str>>().get(1).copied().unwrap();
        for file_name in DEVFS_FILES {
            if file_name == dev {
                let fd = self.fds.len() as u64;
                self.fds.insert(fd, String::from(file_name));
                return Ok(File {
                    fd,
                    r#type: FileType::CharDev,
                });
            }
        }
        if let Some(_dev) = self.devs.get(dev) {
            let fd = self.fds.len() as u64;
            self.fds.insert(fd, String::from(dev));
            return Ok(File {
                fd,
                r#type: FileType::CharDev,
            });
        }
        Err(FsError::NoSuchFile)
    }
    fn read(&mut self, fd: &File, buf: &mut [u8], mut offset: u64) -> Result<u64, FsError> {
        match self.fds.get(&fd.fd) {
            Some(file_name) => match &file_name[..] {
                "zero" => {
                    buf.fill(0);

                    Ok(buf.len() as u64)
                }
                "kmsg" => {
                    let mut buf_off = 0;
                    let kmsg = KMSG.lock();
                    for msg_entry in unsafe { &kmsg.assume_init_ref().msgs } {
                        if buf_off == buf.len() {
                            break;
                        }

                        let msg = msg_entry.to_string();

                        if offset >= msg.len() as u64 {
                            offset -= msg.len() as u64;
                        } else {
                            let read_size =
                                core::cmp::min(buf.len() - buf_off, msg.len() - offset as usize);
                            buf[buf_off..buf_off + read_size].copy_from_slice(
                                &msg.as_bytes()[offset as usize..offset as usize + read_size],
                            );
                            buf_off += read_size;
                            offset = 0;
                        }
                    }

                    Ok(buf_off as u64)
                }
                "random" | "urandom" => {
                    unsafe {
                        (*(&raw mut GLOBAL_RNG)).assume_init_mut().gen_bytes(buf);
                    }
                    Ok(buf.len() as u64)
                }
                _ => Err(FsError::PermissionDenied), // unreadable device
            },
            None => Err(FsError::NoSuchFile),
        }
    }
    fn write(&mut self, fd: &File, buf: &[u8], _offset: u64) -> Result<u64, FsError> {
        match self.fds.get(&fd.fd) {
            Some(file_name) => match &file_name[..] {
                "null" => Ok(buf.len() as u64),
                _ => {
                    if let Some((_major, minor)) = self.devs.get(&file_name[..]) {
                        unsafe {
                            lock_uinit!(DEVICE_MGR).char_devs[*minor]
                                .print_str(&String::from_utf8_lossy(buf));
                        }
                        Ok(buf.len() as u64)
                    } else {
                        Err(FsError::PermissionDenied) // unwritable device
                    }
                }
            },
            None => Err(FsError::NoSuchFile),
        }
    }
    fn remove(&mut self, _path: &Path) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
    fn rename(&mut self, _src: &Path, _dst: &Path) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
    fn close(&mut self, fd: &File) -> Result<(), FsError> {
        self.fds.remove(&fd.fd);
        Ok(())
    }
    fn list_dir(&mut self) -> Result<Vec<String>, FsError> {
        Ok(DEVFS_FILES.map(String::from).to_vec())
    }
    fn mknod(
        &mut self,
        path: &Path,
        _file_type: FileType,
        id: (usize, usize),
    ) -> Result<(), FsError> {
        let dev = path.iter().collect::<Vec<&str>>().first().copied().unwrap();
        self.devs.insert(dev.to_string(), id);
        Ok(())
    }
}

impl DevFS {
    pub fn add_device<S>(&mut self, name: S, id: (usize, usize))
    where
        S: Into<String>,
    {
        let name = name.into();
        if !DEVFS_FILES.contains(&name.as_str()) {
            return;
        }
        self.devs.insert(name, id);
    }
}
