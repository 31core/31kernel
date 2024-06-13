use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{vfs::*, KMSG};
use core::result::Result;

#[derive(Default)]
pub struct DevFS {
    pub fds: BTreeMap<u64, String>,
}

const DEVFS_FILES: [&str; 3] = ["zero", "null", "kmsg"];

impl FileSystem for DevFS {
    fn create(&mut self, _path: &Path) -> Result<File, ()> {
        Err(())
    }
    fn open(&mut self, path: &Path) -> Result<File, ()> {
        for file_name in DEVFS_FILES {
            if file_name == path[0] {
                let fd = self.fds.len() as u64 + 1;
                self.fds.insert(fd, String::from(file_name));
                return Ok(File {
                    fd,
                    r#type: FileType::CharDev,
                });
            }
        }
        Err(())
    }
    fn read(&mut self, fd: &File, buf: &mut [u8], mut offset: u64) -> Result<u64, ()> {
        match self.fds.get(&fd.fd) {
            Some(file_name) => match &file_name[..] {
                "zero" => {
                    for ptr in buf.iter_mut() {
                        *ptr = 0;
                    }

                    Ok(buf.len() as u64)
                }
                "kmsg" => {
                    let mut buf_off = 0;
                    unsafe {
                        for msg in &KMSG.as_ref().unwrap().msgs {
                            if buf_off == buf.len() {
                                break;
                            }

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
                _ => Err(()), // unreadable device
            },
            None => Err(()),
        }
    }
    fn write(&mut self, fd: &File, buf: &[u8]) -> Result<u64, ()> {
        match self.fds.get(&fd.fd) {
            Some(file_name) => match &file_name[..] {
                "null" => Ok(buf.len() as u64),
                _ => Err(()), // unwritable device
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
}
