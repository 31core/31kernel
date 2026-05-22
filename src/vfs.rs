/*! Virtual File System */

use crate::{devfs::DevFS, global::GlobalUninit, mutex::Mutex};
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::{mem::MaybeUninit, result::Result};

pub type Path = [String];

pub static ROOT_VFS: GlobalUninit<VirtualFileSystem> = Mutex::new(MaybeUninit::uninit());

pub fn vfs_init() {
    unsafe {
        let mut rootfs = ROOT_VFS.lock();
        *rootfs = MaybeUninit::new(VirtualFileSystem::default());
        rootfs
            .assume_init_mut()
            .mount(Box::<DevFS>::default(), &[String::from("dev")]);
    }
}

#[derive(Debug)]
pub struct VfsFile {
    pub fd: File,
    pub fs_id: usize,
}

#[derive(Default)]
pub struct VirtualFileSystem {
    max_id: usize,
    pub mount_points: BTreeMap<usize, Vec<String>>,
    pub mounted_fs: BTreeMap<usize, Box<dyn FileSystem>>,
}

unsafe impl Send for VirtualFileSystem {}

impl VirtualFileSystem {
    pub fn open(&mut self, path: &Path) -> Result<VfsFile, ()> {
        let mut found_fs = None;
        let mut found_mountpoint_depth = 0;
        let mut found_fs_id = 0;
        'main: for (fs_id, point) in self.mount_points.iter() {
            for (i, entry) in point.iter().enumerate() {
                if i > path.len() && path[i] != *entry {
                    continue 'main;
                }
            }

            if point.len() > found_mountpoint_depth {
                found_fs_id = *fs_id;
                found_fs = self.mounted_fs.get_mut(fs_id);
                found_mountpoint_depth = point.len();
            }
        }

        if let Some(fs) = found_fs
            && let Ok(fd) = fs.open(&path[found_mountpoint_depth..])
        {
            Ok(VfsFile {
                fd,
                fs_id: found_fs_id,
            })
        } else {
            Err(())
        }
    }
    pub fn mount(&mut self, fs: Box<dyn FileSystem>, mount_point: &Path) {
        self.mounted_fs.insert(self.max_id, fs);
        self.mount_points.insert(self.max_id, mount_point.to_vec());
        self.max_id += 1;
    }
    pub fn umount(&mut self, mount_point: &Path) {
        let mut found_fs_id = None;
        'main: for (fs_id, mpoint) in self.mount_points.iter() {
            for (i, entry) in mount_point.iter().enumerate() {
                if i > mpoint.len() && *entry != mpoint[i] {
                    continue 'main;
                }
            }
            found_fs_id = Some(*fs_id);
            break;
        }
        if let Some(fs_id) = found_fs_id {
            self.mounted_fs.remove(&fs_id);
            self.mount_points.remove(&fs_id);
        }
    }
    pub fn get_fs_mut(&mut self, mount_point: &Path) -> Option<&mut Box<dyn FileSystem>> {
        'main: for (fs_id, mpoint) in self.mount_points.iter() {
            for (i, entry) in mount_point.iter().enumerate() {
                if i > mpoint.len() && *entry != mpoint[i] {
                    continue 'main;
                }
            }
            return self.mounted_fs.get_mut(fs_id);
        }
        None
    }
    pub fn read(&mut self, fd: &VfsFile, buf: &mut [u8], offset: u64) -> Result<u64, ()> {
        self.mounted_fs
            .get_mut(&fd.fs_id)
            .unwrap()
            .read(&fd.fd, buf, offset)
    }
    pub fn write(&mut self, fd: &VfsFile, buf: &[u8]) -> Result<u64, ()> {
        self.mounted_fs
            .get_mut(&fd.fs_id)
            .unwrap()
            .write(&fd.fd, buf)
    }
}

#[derive(Debug)]
pub enum FileType {
    RegularFile,
    Directory,
    CharDev,
    BlockDev,
    SymbolLink,
}

#[derive(Debug)]
pub struct File {
    pub fd: u64,
    pub r#type: FileType,
}

pub trait FileSystem {
    fn create(&mut self, path: &Path) -> Result<File, ()>;
    fn open(&mut self, path: &Path) -> Result<File, ()>;
    fn write(&mut self, fd: &File, buf: &[u8]) -> Result<u64, ()>;
    fn read(&mut self, fd: &File, buf: &mut [u8], offset: u64) -> Result<u64, ()>;
    fn remove(&mut self, path: &Path) -> Result<(), ()>;
    fn rename(&mut self, src: &Path, dst: &Path) -> Result<(), ()>;
    fn close(&mut self, fd: &File) -> Result<(), ()>;
    fn list_dir(&mut self) -> Result<Vec<String>, ()>;
    fn mknod(&mut self, _path: &Path, _file_type: FileType, _id: (usize, usize)) -> Result<(), ()> {
        unimplemented!("mknod is not implemented for this filesystem");
    }
}
