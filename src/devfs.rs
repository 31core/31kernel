/*!
 * devfs filesystem usually mounted on `/dev`
 */

use crate::{
    device::DEVICE_MGR,
    kmsg::KMSG,
    lock_uinit,
    rand::{GLOBAL_RNG, RandomGenerator},
    vfs::{File, FileSystem, FileType, Path},
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
    fn create(&mut self, _path: &Path) -> Result<File, ()> {
        Err(())
    }
    fn open(&mut self, path: &Path) -> Result<File, ()> {
        for file_name in DEVFS_FILES {
            if file_name == path[0] {
                let fd = self.fds.len() as u64;
                self.fds.insert(fd, String::from(file_name));
                return Ok(File {
                    fd,
                    r#type: FileType::CharDev,
                });
            }
        }
        if let Some(_dev) = self.devs.get(&path[0]) {
            let fd = self.fds.len() as u64;
            self.fds.insert(fd, String::from(&path[0]));
            return Ok(File {
                fd,
                r#type: FileType::CharDev,
            });
        }
        Err(())
    }
    fn read(&mut self, fd: &File, buf: &mut [u8], mut offset: u64) -> Result<u64, ()> {
        match self.fds.get(&fd.fd) {
            Some(file_name) => match &file_name[..] {
                "zero" => {
                    buf.fill(0);

                    Ok(buf.len() as u64)
                }
                "kmsg" => {
                    let mut buf_off = 0;
                    let kmsg = KMSG.lock();
                    unsafe {
                        for msg_entry in &kmsg.assume_init_ref().msgs {
                            if buf_off == buf.len() {
                                break;
                            }

                            let msg = msg_entry.to_string();

                            if offset >= msg.len() as u64 {
                                offset -= msg.len() as u64;
                            } else {
                                let read_size = core::cmp::min(buf.len() - buf_off, msg.len());
                                buf[buf_off..buf_off + read_size].copy_from_slice(
                                    &msg.as_bytes()[offset as usize..offset as usize + read_size],
                                );
                                buf_off += read_size;
                            }
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
                _ => Err(()), // unreadable device
            },
            None => Err(()),
        }
    }
    fn write(&mut self, fd: &File, buf: &[u8]) -> Result<u64, ()> {
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
                        Err(()) // unwritable device
                    }
                }
            },
            None => Err(()),
        }
    }
    fn remove(&mut self, _path: &Path) -> Result<(), ()> {
        Err(())
    }
    fn rename(&mut self, _src: &Path, _dst: &Path) -> Result<(), ()> {
        Err(())
    }
    fn close(&mut self, fd: &File) -> Result<(), ()> {
        self.fds.remove(&fd.fd);
        Ok(())
    }
    fn list_dir(&mut self) -> Result<Vec<String>, ()> {
        Ok(DEVFS_FILES.map(String::from).to_vec())
    }
    fn mknod(&mut self, path: &Path, _file_type: FileType, id: (usize, usize)) -> Result<(), ()> {
        self.devs.insert(path[0].clone(), id);
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
