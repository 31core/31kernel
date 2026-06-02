/*! Virtual File System */

use crate::{
    devfs::DevFS,
    global::GlobalUninit,
    mutex::Mutex,
    path::{Path, PathBuf},
};
use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::{mem::MaybeUninit, result::Result};

pub static ROOT_VFS: GlobalUninit<VirtualFileSystem> = Mutex::new(MaybeUninit::uninit());

pub fn vfs_init() {
    unsafe {
        let mut rootfs = ROOT_VFS.lock();
        *rootfs = MaybeUninit::new(VirtualFileSystem::default());
        rootfs
            .assume_init_mut()
            .mount(Box::<DevFS>::default(), "/dev");
    }
}

#[derive(Debug)]
pub enum VfsError {
    NotMounted,
    FsError(FsError),
}

#[derive(Debug)]
pub enum FsError {
    NoSuchFile,
    NotSupported,
    PermissionDenied,
    Other,
}

#[derive(Debug)]
pub struct VfsFile {
    pub fd: File,
    pub offset: u64,
    pub fs_id: usize,
}

#[derive(Default)]
pub struct VirtualFileSystem {
    max_id: usize,
    pub mount_points: BTreeMap<usize, PathBuf>,
    pub mounted_fs: BTreeMap<usize, Box<dyn FileSystem>>,
}

unsafe impl Send for VirtualFileSystem {}

impl VirtualFileSystem {
    pub fn open<P>(&mut self, path: P) -> Result<VfsFile, VfsError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        debug_assert!(path.is_absolute());

        let mut found_fs = None;
        let mut found_fs_id = 0;
        let mut found_mount_point = None;
        for (fs_id, mpoint) in self.mount_points.iter() {
            if path.starts_with(mpoint) {
                found_fs_id = *fs_id;
                found_fs = self.mounted_fs.get_mut(fs_id);
                found_mount_point = Some(mpoint);
                break;
            }
        }

        if let Some(fs) = found_fs {
            let prefix = found_mount_point.unwrap();
            match fs.open(&path.to_owned().strip_prefix(prefix).unwrap()) {
                Ok(fd) => Ok(VfsFile {
                    fd,
                    offset: 0,
                    fs_id: found_fs_id,
                }),
                Err(err) => Err(VfsError::FsError(err)),
            }
        } else {
            Err(VfsError::NotMounted)
        }
    }
    pub fn mount<P>(&mut self, fs: Box<dyn FileSystem>, mount_point: P)
    where
        P: AsRef<Path>,
    {
        let mount_point = mount_point.as_ref();
        debug_assert!(mount_point.is_absolute());

        self.mounted_fs.insert(self.max_id, fs);
        self.mount_points
            .insert(self.max_id, mount_point.to_owned());
        self.max_id += 1;
    }
    pub fn umount<P>(&mut self, mount_point: P)
    where
        P: AsRef<Path>,
    {
        let mut found_fs_id = None;
        for (fs_id, mpoint) in self.mount_points.iter() {
            if mount_point.as_ref().starts_with(mpoint) {
                found_fs_id = Some(*fs_id);
                break;
            }
        }
        if let Some(fs_id) = found_fs_id {
            self.mounted_fs.remove(&fs_id);
            self.mount_points.remove(&fs_id);
        }
    }
    pub fn get_fs_mut<P>(&mut self, mount_point: P) -> Option<&mut Box<dyn FileSystem>>
    where
        P: AsRef<Path>,
    {
        for (fs_id, mpoint) in self.mount_points.iter() {
            if mount_point.as_ref().starts_with(mpoint) {
                return self.mounted_fs.get_mut(fs_id);
            }
        }
        None
    }
    pub fn read(&mut self, fd: &mut VfsFile, buf: &mut [u8]) -> Result<u64, FsError> {
        match self
            .mounted_fs
            .get_mut(&fd.fs_id)
            .unwrap()
            .read(&fd.fd, buf, fd.offset)
        {
            Ok(size) => {
                fd.offset += size;
                Ok(size)
            }
            Err(err) => Err(err),
        }
    }
    pub fn write(&mut self, fd: &mut VfsFile, buf: &[u8]) -> Result<u64, FsError> {
        match self
            .mounted_fs
            .get_mut(&fd.fs_id)
            .unwrap()
            .write(&fd.fd, buf, fd.offset)
        {
            Ok(size) => {
                fd.offset += size;
                Ok(size)
            }
            Err(err) => Err(err),
        }
    }
    pub fn close(&mut self, fd: &VfsFile) -> Result<(), FsError> {
        self.mounted_fs.get_mut(&fd.fs_id).unwrap().close(&fd.fd)
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
    fn create(&mut self, path: &Path) -> Result<File, FsError>;
    fn open(&mut self, path: &Path) -> Result<File, FsError>;
    fn write(&mut self, fd: &File, buf: &[u8], offset: u64) -> Result<u64, FsError>;
    fn read(&mut self, fd: &File, buf: &mut [u8], offset: u64) -> Result<u64, FsError>;
    fn remove(&mut self, path: &Path) -> Result<(), FsError>;
    fn rename(&mut self, src: &Path, dst: &Path) -> Result<(), FsError>;
    fn close(&mut self, fd: &File) -> Result<(), FsError>;
    fn list_dir(&mut self) -> Result<Vec<String>, FsError>;
    fn mknod(
        &mut self,
        _path: &Path,
        _file_type: FileType,
        _id: (usize, usize),
    ) -> Result<(), FsError> {
        Err(FsError::NotSupported)
    }
}
